//! Lifecycle regressions over real Owner/Pool/Inbox and actual direct
//! acknowledgement. Only source/model inputs occupy the explicit synthetic
//! test domain; no successful Observation or native callback is fabricated.
use super::*;
use std::{sync::{Arc,mpsc},thread,time::Duration};
use openmt2001::camera::bank::Bank;
use super::tests::{Ui,capture};
use super::super::ui_selection::{SelectionState,RelatedKey};

fn fixture_assets()->crate::selection::Assets{
    crate::selection::Assets::descriptor_test_assets(vec![],[0;116])
}
fn source(epoch:u64,assets:&crate::selection::Assets)->Arc<bridge::PropertySource>{
    let mut rows=[[0;380];52];
    for(i,row)in rows.iter_mut().enumerate(){
        row[0..4].copy_from_slice(&(i as u32).to_le_bytes());
        row[0x94..0x98].copy_from_slice(&(0xff004000+i as u32).to_le_bytes());
    }
    Arc::new(bridge::PropertySource::synthetic(epoch,rows,52,assets))
}
fn selection()->SelectionState{
    SelectionState::installed(None,None,None,RelatedKey::from_native_cell(0x702000),
        Some(0x703000),Some(9),12,13)
}
fn make_endpoint(owner:Owner,epoch:u64,assets:&crate::selection::Assets)->(Endpoint,bridge::Worker){
    let (worker,inbox)=bridge::channel(epoch);
    (Endpoint::with_property_install(owner,inbox,PropertyInstall::synthetic(assets)),worker)
}
fn owner()->Owner{
    let mut owner=Owner::new(Bank::loader_zero_storage(),0xa5);
    owner.install_ui_selection(selection()).unwrap();
    owner
}
/// No fake ack lambda: the real endpoint allocates and validates the Pool.
fn exchange(endpoint:&mut Endpoint,mut worker:bridge::Worker,property:Option<Arc<bridge::PropertySource>>)
    ->(bridge::Worker,Result<Observation,bridge::RequestError>,Result<Service,String>){
    let (sent,held)=mpsc::channel();
    let handle=thread::spawn(move||{
        let result=worker.request(Operation::Install{property,capture:capture(),admission:Admission::default()});
        sent.send(()).unwrap();
        (worker,result)
    });
    let mut context=Ui::default();
    let service=endpoint.wait_service(&mut context,Duration::from_secs(2));
    // Timeout is test failure, never a replacement success/ack.
    held.recv_timeout(Duration::from_secs(2)).expect("worker must receive actual direct ack");
    let(worker,result)=handle.join().unwrap();
    (worker,result,service)
}
pub(crate) fn install(epoch:u64,assets:&crate::selection::Assets)->(Endpoint,bridge::Worker){
    let(mut endpoint,worker)=make_endpoint(owner(),epoch,assets);
    let(worker,result,service)=exchange(&mut endpoint,worker,Some(source(epoch,assets)));
    assert_eq!(service,Ok(Service::Completed));result.unwrap();
    assert!(worker.installed());
    assert_eq!(endpoint.owner().property_descriptors().unwrap().count().unwrap(),52);
    assert!(endpoint.camera_only_admission());
    (endpoint,worker)
}
#[test]
fn install_requires_completed_source_before_any_raw_or_ui_write(){
    let assets=fixture_assets();
    let(mut endpoint,worker)=make_endpoint(owner(),501,&assets);
    let before=endpoint.capture();let dirty=endpoint.owner().kind_dirty();
    let(worker,result,service)=exchange(&mut endpoint,worker,None);
    assert!(result.is_err()&&service.is_err());assert!(!worker.installed());
    assert_eq!(endpoint.capture(),before);assert_eq!(endpoint.owner().kind_dirty(),dirty);
    assert!(endpoint.owner().property_lifecycle_error().unwrap().contains("completed Session"));
    assert!(endpoint.owner().ui_selection().unwrap().current.is_none());
}
#[test]
fn wrong_epoch_or_resource_is_rejected_before_camera_ui_mutation_and_latched(){
    let assets=fixture_assets();let foreign=fixture_assets();
    for property in [source(500,&assets),source(501,&foreign)]{
        let(mut endpoint,worker)=make_endpoint(owner(),501,&assets);
        let before=endpoint.capture();let dirty=endpoint.owner().kind_dirty();
        let(mut worker,result,service)=exchange(&mut endpoint,worker,Some(property));
        assert!(result.is_err()&&service.is_err());
        assert_eq!(endpoint.capture(),before);assert_eq!(endpoint.owner().kind_dirty(),dirty);
        assert!(endpoint.owner().property_lifecycle_error().is_some());
        assert!(worker.request(Operation::CurrentPose).is_err());
        assert!(endpoint.owner().ui_selection().unwrap().current.is_none());
    }
}
#[test]
fn actual_retirement_precedes_worker_wake_and_new_world_reuses_same_pool(){
    let assets=fixture_assets();
    let(mut endpoint,mut worker)=install(601,&assets);
    let old=endpoint.owner.property_descriptors().unwrap().lookup(0).unwrap().unwrap();
    endpoint.owner.property_lookup_current(0).unwrap();
    let current=endpoint.owner.current_ui_selected().unwrap();
    let published=Some(endpoint.owner.property_descriptors().unwrap().selected(
        endpoint.owner.property_descriptors().unwrap().lookup(1).unwrap().unwrap()).unwrap());
    endpoint.owner.ui_selection_mut().unwrap().published=published;
    let retained=Some(endpoint.owner.property_descriptors().unwrap().selected(
        endpoint.owner.property_descriptors().unwrap().lookup(2).unwrap().unwrap()).unwrap());
    let identity=endpoint.owner.property_descriptors().unwrap().test_identity();
    let before=endpoint.capture();let dirty=endpoint.owner().kind_dirty();
    let(queued,started)=mpsc::channel();let(woken,awake)=mpsc::channel();
    let retired_flag=worker.test_retirement_flag();
    worker.test_notify_next_enqueue(queued);
    let handle=thread::spawn(move||{
        let result=worker.request(Operation::CurrentPose);
        woken.send(result.clone()).unwrap();
        (worker,result)
    });
    started.recv_timeout(Duration::from_secs(2)).expect("actual Worker send must enqueue before retirement");
    assert!(!retired_flag.load(std::sync::atomic::Ordering::Acquire));
    // The actual ordered retirement helper retires Pool before publishing its
    // retirement flag and dropping direct reply senders. No request is serviced.
    let owner=endpoint.retire_into_owner(|owner|{
        assert!(owner.property_descriptors().unwrap().resolve(old).is_err());
        for value in [current,published,retained]{assert!(owner.validate_ui_selected(value).is_err());}
        assert!(!retired_flag.load(std::sync::atomic::Ordering::Acquire),
            "Pool invalidation must precede actual Inbox retirement publication");
        assert_eq!(owner.capture(),before);assert_eq!(owner.kind_dirty(),dirty);
    });
    assert!(retired_flag.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(awake.recv_timeout(Duration::from_secs(2)).unwrap(),Err(bridge::RequestError::Retired));
    let(_worker,result)=handle.join().unwrap();assert_eq!(result,Err(bridge::RequestError::Retired));
    assert_eq!(owner.capture(),before);assert_eq!(owner.kind_dirty(),dirty);
    assert!(owner.property_descriptors().unwrap().resolve(old).is_err());
    assert!(owner.validate_ui_selected(current).is_err());
    assert!(owner.validate_ui_selected(published).is_err());
    assert!(owner.validate_ui_selected(retained).is_err());
    assert_eq!(owner.ui_selection().unwrap().current,current);
    assert_eq!(owner.ui_selection().unwrap().published,published);
    let retired=owner.property_descriptors().unwrap().test_identity();
    assert_eq!(identity.0,retired.0);assert!(retired.1>identity.1);
    let(mut next,worker)=make_endpoint(owner,602,&assets);
    let(_worker,result,service)=exchange(&mut next,worker,Some(source(602,&assets)));
    assert_eq!(service,Ok(Service::Completed));result.unwrap();
    let rebuilt=next.owner.property_descriptors().unwrap().test_identity();
    assert_eq!(rebuilt.0,identity.0);assert!(rebuilt.1>retired.1);
    assert_ne!(next.owner.property_descriptors().unwrap().lookup(0).unwrap().unwrap(),old);
    // No stale facade repair by category, and distinct cells retain their writes.
    assert_eq!(next.owner.ui_selection().unwrap().current,current);
    assert_eq!(next.owner.ui_selection().unwrap().published,published);
    for value in [current,published,retained]{assert!(next.owner.validate_ui_selected(value).is_err());}
}
#[test]
fn old_epoch_cannot_rebind_after_new_channel_or_after_retirement(){
    let assets=fixture_assets();
    let(endpoint,_worker)=install(701,&assets);
    let owner=endpoint.into_owner();
    let(mut next,worker)=make_endpoint(owner,701,&assets);
    let before=next.capture();let dirty=next.owner.kind_dirty();
    let(_worker,result,service)=exchange(&mut next,worker,Some(source(701,&assets)));
    assert!(result.is_err()&&service.is_err());
    assert_eq!(next.capture(),before);assert_eq!(next.owner.kind_dirty(),dirty);
    assert!(next.owner.property_descriptors().is_err(),"stale install fault is latched");
}
#[test]
fn revoked_model_and_failed_allocation_leave_retired_pool_and_latched_error(){
    for revoked in [false,true]{
        let mut assets=fixture_assets();
        let(endpoint,_worker)=install(801,&assets);
        let mut owner=endpoint.into_owner();
        let identity=owner.property_descriptors().unwrap().test_identity();
        owner.property_descriptors_mut().unwrap().test_fail_allocation();
        let property=source(802,&assets);
        let resources=PropertyInstall::synthetic(&assets);
        if revoked{assets.retire_pavement_resource().unwrap();}
        let(worker,inbox)=bridge::channel(802);
        let mut next=Endpoint::with_property_install(owner,inbox,resources);
        let before=next.capture();let dirty=next.owner.kind_dirty();
        let(_worker,result,service)=exchange(&mut next,worker,Some(property));
        assert!(result.is_err()&&service.is_err());
        assert_eq!(next.capture(),before);assert_eq!(next.owner.kind_dirty(),dirty);
        let fault=next.owner.property_lifecycle_error().unwrap().to_owned();
        assert!(if revoked{fault.contains("resource")}else{fault.contains("allocation failed")});
        // Pool remains owned even though public reads correctly expose the fault.
        // Retirement again preserves the fault and does not revive any alias.
        let parked=next.into_owner();
        assert_eq!(parked.property_lifecycle_error(),Some(fault.as_str()));
        assert_eq!(identity.0,parked.test_retained_pool_identity().unwrap().0);
        assert!(parked.test_retained_pool_is_retired());
    }
}
#[test]
fn generation_exhaustion_is_reported_before_parking_and_cannot_reanimate(){
    let assets=fixture_assets();
    let(mut endpoint,_worker)=install(901,&assets);
    endpoint.owner.property_descriptors_mut().unwrap().test_exhaust_generation();
    let before=endpoint.capture();let dirty=endpoint.owner.kind_dirty();
    let owner=endpoint.into_owner();
    assert_eq!(owner.property_lifecycle_error(),Some("descriptor generation exhausted"));
    assert!(owner.test_retained_pool_is_retired());
    assert_eq!(owner.capture(),before);assert_eq!(owner.kind_dirty(),dirty);
    let(mut next,worker)=make_endpoint(owner,902,&assets);
    let(_worker,result,service)=exchange(&mut next,worker,Some(source(902,&assets)));
    assert!(result.is_err()&&service.is_err());
    assert_eq!(next.owner.property_lifecycle_error(),Some("descriptor generation exhausted"));
}

#[test]
fn wrong_count_from_install_is_not_a_pool_replacement_or_camera_write(){
    let assets=fixture_assets();
    let(endpoint,_worker)=install(1001,&assets);
    let owner=endpoint.into_owner();
    let identity=owner.test_retained_pool_identity().unwrap();
    let(mut next,worker)=make_endpoint(owner,1002,&assets);
    let malformed=Arc::new(bridge::PropertySource::synthetic(1002,[[0;380];52],53,&assets));
    let before=next.capture();let dirty=next.owner.kind_dirty();
    let(_worker,result,service)=exchange(&mut next,worker,Some(malformed));
    assert!(result.is_err()&&service.is_err());
    assert!(next.owner.property_lifecycle_error().unwrap().contains("count"));
    assert_eq!(next.owner.test_retained_pool_identity().unwrap(),identity);
    assert!(next.owner.test_retained_pool_is_retired());
    assert_eq!(next.capture(),before);assert_eq!(next.owner.kind_dirty(),dirty);
}

#[test]
fn retained_pool_rejects_old_protected_epoch_even_when_all_rows_match(){
    let assets=fixture_assets();
    let model=super::super::property_descriptors::PavementModel::bind(&assets).unwrap();
    let reset=super::super::property_descriptors::ResetTransform::synthetic();
    let mut pool=super::super::property_descriptors::Pool::build_from_install(&source(1101,&assets),&model,&reset).unwrap();
    pool.retire_source_frame().unwrap();
    pool.rebuild_from_install(&source(1102,&assets),&model,&reset).unwrap();
    let current=pool.lookup(0).unwrap().unwrap();
    let old=crate::navigator::upper::Inputs{
        source_epoch:bridge::PropertyEpoch::synthetic(1101),
        rows:*source(1101,&assets).rows(),owners:[None;52],count:52,phase:0,
        presentation_mode:0,property_labels:[0;52],
    };
    assert!(pool.rebuild_from_protected(&old,&model,&reset).unwrap_err().contains("epoch"));
    assert!(pool.test_is_retired());assert!(pool.resolve(current).is_err());
}

fn protected_source(epoch:u64,assets:&crate::selection::Assets)->crate::navigator::upper::Inputs{
    crate::navigator::upper::Inputs{
        source_epoch:bridge::PropertyEpoch::synthetic(epoch),
        rows:*source(epoch,assets).rows(),owners:[None;52],count:52,phase:0,
        presentation_mode:0,property_labels:[0;52],
    }
}
#[test]
fn live_protected_rebuild_is_allowed_but_actual_endpoint_retirement_blocks_same_and_lower_epoch(){
    let assets=fixture_assets();
    let model=super::super::property_descriptors::PavementModel::bind(&assets).unwrap();
    let reset=super::super::property_descriptors::ResetTransform::synthetic();
    let(mut endpoint,_worker)=install(1201,&assets);
    let input=protected_source(1201,&assets);
    let before=endpoint.capture();let dirty=endpoint.owner.kind_dirty();
    let identity=endpoint.owner.property_descriptors().unwrap().test_identity();
    let old=endpoint.owner.property_descriptors().unwrap().lookup(0).unwrap().unwrap();
    endpoint.owner.property_descriptors_mut().unwrap().rebuild_from_protected(&input,&model,&reset).unwrap();
    let live=endpoint.owner.property_descriptors().unwrap();
    let replacement=live.lookup(0).unwrap().unwrap();
    assert_eq!(live.test_identity().0,identity.0);
    assert!(live.test_identity().1>identity.1);assert!(live.resolve(old).is_err());
    // Exercise the real Endpoint -> Owner retirement boundary, not Pool alone.
    let mut parked=endpoint.into_owner();
    assert!(parked.property_lifecycle_error().is_none());
    for detached in [&input,&protected_source(1200,&assets)]{
        let pool=parked.property_descriptors_mut().unwrap();
        assert!(pool.rebuild_from_protected(detached,&model,&reset).unwrap_err().contains("epoch"));
        assert!(pool.test_is_retired());assert!(pool.lookup(0).is_err());
        assert!(pool.resolve(old).is_err());assert!(pool.resolve(replacement).is_err());
        assert_eq!(pool.test_identity().0,identity.0);
    }
    assert_eq!(parked.capture(),before);assert_eq!(parked.kind_dirty(),dirty);
    let(mut next,worker)=make_endpoint(parked,1202,&assets);
    let(_worker,result,service)=exchange(&mut next,worker,Some(source(1202,&assets)));
    assert_eq!(service,Ok(Service::Completed));result.unwrap();
    let pool=next.owner.property_descriptors().unwrap();
    assert_eq!(pool.test_identity().0,identity.0);assert_eq!(pool.count().unwrap(),52);
    assert!(pool.resolve(old).is_err());assert!(pool.resolve(replacement).is_err());
}
#[test]
fn failed_protected_replacement_consumes_epoch_and_detached_same_epoch_retry_stays_retired(){
    let assets=fixture_assets();
    let model=super::super::property_descriptors::PavementModel::bind(&assets).unwrap();
    let reset=super::super::property_descriptors::ResetTransform::synthetic();
    let(mut endpoint,_worker)=install(1301,&assets);
    let old=endpoint.owner.property_descriptors().unwrap().lookup(0).unwrap().unwrap();
    let before=endpoint.capture();let dirty=endpoint.owner.kind_dirty();
    let pool=endpoint.owner.property_descriptors_mut().unwrap();
    let identity=pool.test_identity();
    pool.test_fail_allocation();
    let input=protected_source(1302,&assets);
    assert!(pool.rebuild_from_protected(&input,&model,&reset).unwrap_err().contains("allocation failed"));
    assert!(pool.test_is_retired());assert!(pool.resolve(old).is_err());
    for detached in [&input,&protected_source(1301,&assets)]{
        assert!(pool.rebuild_from_protected(detached,&model,&reset).unwrap_err().contains("epoch"));
        assert!(pool.test_is_retired());assert!(pool.lookup(0).is_err());
        assert_eq!(pool.test_identity().0,identity.0);
    }
    assert_eq!(endpoint.capture(),before);assert_eq!(endpoint.owner.kind_dirty(),dirty);
    // Owner authority still requires its real retirement and a new install.
    let parked=endpoint.into_owner();
    let(mut next,worker)=make_endpoint(parked,1303,&assets);
    let(_worker,result,service)=exchange(&mut next,worker,Some(source(1303,&assets)));
    assert_eq!(service,Ok(Service::Completed));result.unwrap();
    let pool=next.owner.property_descriptors().unwrap();
    assert_eq!(pool.test_identity().0,identity.0);assert_eq!(pool.count().unwrap(),52);
    assert!(pool.resolve(old).is_err());
}
#[test]
fn failed_live_same_epoch_rebuild_cannot_retry_with_its_detached_input(){
    let assets=fixture_assets();
    let model=super::super::property_descriptors::PavementModel::bind(&assets).unwrap();
    let reset=super::super::property_descriptors::ResetTransform::synthetic();
    let input=protected_source(1401,&assets);
    let mut pool=super::super::property_descriptors::Pool::build_from_protected(&input,&model,&reset).unwrap();
    let old=pool.lookup(0).unwrap().unwrap();
    pool.test_fail_allocation();
    assert!(pool.rebuild_from_protected(&input,&model,&reset).unwrap_err().contains("allocation failed"));
    assert!(pool.test_is_retired());assert!(pool.resolve(old).is_err());
    assert!(pool.rebuild_from_protected(&input,&model,&reset).unwrap_err().contains("epoch"));
    assert!(pool.test_is_retired());assert!(pool.lookup(0).is_err());
}
