//! Real registry/Owner/allocation regressions with explicit synthetic epochs.
use super::*;
use openmt2001::{resident::AuxiliaryPool,camera::bank::Bank};
use super::super::{owner::Owner,ui_selection::SelectionState};
fn owner()->Owner {
    let mut owner=Owner::new(Bank::loader_zero_storage(),0xa5);
    owner.install_ui_selection(SelectionState::installed(None,None,None,None,None,None,-1,-1)).unwrap();owner
}
#[test]
fn real_registry_rebinds_stably_and_keeps_selection_cells_distinct() {
    let mut pool=AuxiliaryPool::from_slots(vec![[0;284];3],1).unwrap();
    let source=ResidentSource::synthetic(910,&pool);let mut owner=owner();
    owner.install_resident_source(&source,910).unwrap();
    let binding=pool.bind(0).unwrap();
    let value=owner.resident_descriptors_mut().unwrap().selected(binding.clone(),2,777).unwrap();
    let same=owner.resident_descriptors_mut().unwrap().selected(binding.clone(),-3,4).unwrap();
    assert_eq!(value.selected().key,same.selected().key);
    assert!(owner.resolve_ui_selected(Some(value.selected())).err().unwrap().contains("same-operation Host"));
    let key=value.selected().key;assert!(key.native_key().is_err());
    assert!(matches!(key,SelectionKey::ResidentDescriptor(_)));
    owner.ui_selection_mut().unwrap().current=Some(value.selected());
    owner.ui_selection_mut().unwrap().publish_current();
    owner.ui_selection_mut().unwrap().transient=Some(same.selected());
    owner.ui_selection_mut().unwrap().current=None;
    assert_eq!(owner.published_ui_selected().unwrap(),Some(value.selected()));
    assert_eq!(owner.transient_ui_selected().unwrap(),Some(same.selected()));
    pool.allocate(&[1;284]).unwrap();owner.resident_descriptors().unwrap().validate_selected(value.selected()).unwrap();
    pool.reset_for_residents();
    assert!(owner.published_ui_selected().is_err());assert!(owner.transient_ui_selected().is_err());
    // Raw source cells retain their copies; copied kind/category never restores authority.
    assert_eq!(owner.ui_selection().unwrap().published.unwrap().kind,2);
    assert_eq!(owner.ui_selection().unwrap().transient.unwrap().kind,-3);
}
#[test]
fn retire_keeps_process_identity_and_rejects_detached_same_epoch_cross_world_and_pool() {
    let pool=AuxiliaryPool::from_slots(vec![[7;284]],1).unwrap();
    let source=ResidentSource::synthetic(920,&pool);let mut registry=Registry::new().unwrap();
    registry.install(&source,920).unwrap();let identity=registry.identity();
    let first=registry.selected(pool.bind(0).unwrap(),2,0).unwrap();
    let SelectionKey::ResidentDescriptor(handle)=first.selected().key else {panic!()};
    let mut foreign=Registry::new().unwrap();foreign.install(&source,920).unwrap();
    assert!(foreign.binding(handle).is_err());
    let other=AuxiliaryPool::from_slots(vec![[7;284]],1).unwrap();
    assert!(registry.selected(other.bind(0).unwrap(),2,0).is_err());
    registry.retire();assert!(registry.binding(handle).is_err());
    assert_eq!(registry.identity(),(identity.0,identity.1+1));
    assert!(registry.install(&source,920).is_err());
    let next=ResidentSource::synthetic(921,&other);registry.install(&next,921).unwrap();
    assert_eq!(registry.identity().0,identity.0);
    assert!(registry.binding(handle).is_err());assert!(registry.selected(pool.bind(0).unwrap(),2,0).is_err());
    let current=registry.selected(other.bind(0).unwrap(),2,0).unwrap();assert_ne!(first.selected().key,current.selected().key);
}
#[test]
fn wrong_channel_and_drop_are_rejected_before_selection_publication() {
    let pool=AuxiliaryPool::from_slots(vec![[0;284]],1).unwrap();
    let source=ResidentSource::synthetic(930,&pool);let mut owner=owner();
    assert!(owner.install_resident_source(&source,931).is_err());assert!(owner.ui_selection().unwrap().current.is_none());
    owner.install_resident_source(&source,930).unwrap();let binding=pool.bind(0).unwrap();
    drop(pool);assert!(owner.resident_descriptors_mut().unwrap().selected(binding,2,0).is_err());
    assert!(owner.ui_selection().unwrap().current.is_none());
    assert!(owner.resident_descriptors().unwrap().validate_selected(Selected{key:SelectionKey::NativeObject(0x6899a0),kind:2,category:0,hit:None}).is_err());
}
