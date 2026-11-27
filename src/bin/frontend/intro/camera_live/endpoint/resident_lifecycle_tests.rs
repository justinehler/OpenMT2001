//! Actual Endpoint Install and pre-wake retirement regressions.
use super::super::ui_selection::{SelectionKey, SelectionState};
use super::tests::{Ui, capture};
use super::*;
use openmt2001::{camera::bank::Bank, resident::AuxiliaryPool};
use std::{sync::Arc, thread, time::Duration};
fn make_endpoint(
    owner: Owner,
    epoch: u64,
    assets: &crate::selection::Assets,
) -> (Endpoint, bridge::Worker) {
    let (worker, inbox) = bridge::channel(epoch);
    (
        Endpoint::with_property_install(owner, inbox, PropertyInstall::synthetic(assets)),
        worker,
    )
}
fn source(
    epoch: u64,
    assets: &crate::selection::Assets,
    pool: &AuxiliaryPool,
) -> Arc<bridge::PropertySource> {
    let mut rows = [[0; 380]; 52];
    for (i, row) in rows.iter_mut().enumerate() {
        row[..4].copy_from_slice(&(i as u32).to_le_bytes());
    }
    Arc::new(
        bridge::PropertySource::synthetic(epoch, rows, 52, assets)
            .test_with_resident(bridge::ResidentSource::synthetic(epoch, pool)),
    )
}
fn install(
    endpoint: &mut Endpoint,
    mut worker: bridge::Worker,
    source: Arc<bridge::PropertySource>,
) -> bridge::Worker {
    let client = thread::spawn(move || {
        worker
            .request(Operation::Install {
                property: Some(source),
                capture: capture(),
                admission: Admission::default(),
            })
            .unwrap();
        worker
    });
    endpoint
        .wait_service(&mut Ui::default(), Duration::from_secs(3))
        .unwrap();
    client.join().unwrap()
}
#[test]
fn actual_install_retire_pre_wake_and_next_world_retain_same_registry() {
    let assets = crate::selection::Assets::descriptor_test_assets(vec![], [0; 116]);
    let mut owner = Owner::new(Bank::loader_zero_storage(), 0xa5);
    owner
        .install_ui_selection(SelectionState::installed(
            None, None, None, None, None, None, -1, -1,
        ))
        .unwrap();
    let pool = AuxiliaryPool::from_slots(vec![[0; 284]], 1).unwrap();
    let captured = source(940, &assets, &pool);
    let (mut endpoint, worker) = make_endpoint(owner, 940, &assets);
    let mut worker = install(&mut endpoint, worker, captured.clone());
    let identity = endpoint.owner.resident_descriptors().unwrap().identity();
    let descriptor = endpoint
        .owner
        .resident_descriptors_mut()
        .unwrap()
        .selected(pool.bind(0).unwrap(), 2, 0)
        .unwrap();
    endpoint.owner.ui_selection_mut().unwrap().current = Some(descriptor.selected());
    endpoint.owner.ui_selection_mut().unwrap().publish_current();
    endpoint.owner.ui_selection_mut().unwrap().transient = Some(descriptor.selected());
    let flag = worker.test_retirement_flag();
    let (queued, started) = std::sync::mpsc::channel();
    worker.test_notify_next_enqueue(queued);
    let waiting = thread::spawn(move || worker.request(Operation::CurrentPose));
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let owner = endpoint.retire_into_owner(|owner| {
        assert!(!flag.load(std::sync::atomic::Ordering::Acquire));
        assert!(owner.resident_descriptors().unwrap().source().is_err());
        assert!(owner.current_ui_selected().is_err());
        assert!(owner.published_ui_selected().is_err());
        assert!(owner.transient_ui_selected().is_err());
    });
    assert!(flag.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(waiting.join().unwrap(), Err(bridge::RequestError::Retired));
    assert!(
        owner
            .validate_resident_install(captured.resident().unwrap(), 940)
            .is_err()
    );
    let next_pool = AuxiliaryPool::from_slots(vec![[0; 284]], 1).unwrap();
    let (mut next, worker) = make_endpoint(owner, 941, &assets);
    let _worker = install(&mut next, worker, source(941, &assets, &next_pool));
    assert_eq!(
        next.owner.resident_descriptors().unwrap().identity().0,
        identity.0
    );
    let SelectionKey::ResidentDescriptor(old) = descriptor.selected().key else {
        panic!()
    };
    assert!(
        next.owner
            .resident_descriptors()
            .unwrap()
            .binding(old)
            .is_err()
    );
    // This mirrors the existing refresh move: no source re-install/rebind.
    let mut parked = Some(next);
    let moved = parked.take().unwrap();
    assert_eq!(
        moved.owner.resident_descriptors().unwrap().identity().0,
        identity.0
    );
    moved.into_owner();
}

#[test]
fn mismatched_resident_source_rejects_actual_install_before_any_restore() {
    let assets = crate::selection::Assets::descriptor_test_assets(vec![], [0; 116]);
    let owner = Owner::new(Bank::loader_zero_storage(), 0xa5);
    let pool = AuxiliaryPool::from_slots(vec![[0; 284]], 1).unwrap();
    let mut rows = [[0; 380]; 52];
    for (i, row) in rows.iter_mut().enumerate() {
        row[..4].copy_from_slice(&(i as u32).to_le_bytes());
    }
    let captured = Arc::new(
        bridge::PropertySource::synthetic(950, rows, 52, &assets)
            .test_with_resident(bridge::ResidentSource::synthetic(951, &pool)),
    );
    let (mut endpoint, mut worker) = make_endpoint(owner, 950, &assets);
    let before = endpoint.capture();
    let client = thread::spawn(move || {
        worker.request(Operation::Install {
            property: Some(captured),
            capture: capture(),
            admission: Admission::default(),
        })
    });
    let result = endpoint.wait_service(&mut Ui::default(), Duration::from_secs(3));
    assert!(result.unwrap_err().contains("epoch mismatch"));
    assert!(client.join().unwrap().is_err());
    assert_eq!(endpoint.capture(), before);
    assert_eq!(endpoint.owner.kind_dirty(), 0xa5);
    assert!(endpoint.owner.resident_descriptors().is_err());
}

/// Shared real Endpoint install for the production refresh transaction test.
pub(crate) fn installed_for_refresh(
    epoch: u64,
    assets: &crate::selection::Assets,
) -> (Endpoint, bridge::Worker, AuxiliaryPool) {
    let pool = AuxiliaryPool::from_slots(vec![[0; 284]], 1).unwrap();
    let mut owner = Owner::new(Bank::loader_zero_storage(), 0xa5);
    owner
        .install_ui_selection(SelectionState::installed(
            None, None, None, None, None, None, -1, -1,
        ))
        .unwrap();
    let (mut endpoint, worker) = make_endpoint(owner, epoch, assets);
    let worker = install(&mut endpoint, worker, source(epoch, assets, &pool));
    let selected = endpoint
        .owner
        .resident_descriptors_mut()
        .unwrap()
        .selected(pool.bind(0).unwrap(), 2, 0)
        .unwrap()
        .selected();
    endpoint.owner.ui_selection_mut().unwrap().current = Some(selected);
    endpoint.owner.ui_selection_mut().unwrap().published = Some(selected);
    endpoint.owner.ui_selection_mut().unwrap().transient = Some(selected);
    (endpoint, worker, pool)
}
