//! Production startup/writer/read code without a parallel model.
use super::*;
pub(super) fn loader() -> Vec<u8> {
    synthetic_loader()
}
pub(crate) fn fixture() -> Handle {
    test_fixture()
}
#[test]
fn actual_startup_loader_defaults_constructors_are_distinct_products() {
    let (p, activity) = startup(&loader(), "").unwrap();
    for byte in [
        Byte::QuitOnEscape8b23b9,
        Byte::Debug8b23c5,
        Byte::Test8b23bc,
        Byte::Network93bbb9,
        Byte::Alternative8fca96,
        Byte::Suppressed8b0700,
    ] {
        assert_eq!(p.byte(byte).unwrap(), 0);
    }
    assert_eq!(p.byte(Byte::Controller920888).unwrap(), 1);
    assert_eq!(
        p.picking_projection().unwrap(),
        PickingProjection {
            common: 1,
            camera: 1,
            block: 0,
            block_id: -1,
            building: 0,
            building_id: -1
        }
    );
    assert_eq!(p.count822d38().unwrap(), 0);
    assert_eq!((activity.a4(), activity.a8()), (0, 0));
    assert_eq!(
        p.media_producer().unwrap(),
        MediaProducer::Awaiting(MediaSite::Startup4b94eb)
    );
    assert_eq!(p.byte(Byte::Media8fca98), Err(Error::MediaProbeUnavailable));
}
#[test]
fn actual_native_parser_defaults_and_delimiters_retain_options_on_same_handle() {
    let (p, _) = startup(&loader(), " -debugstuff,+TeStMoDe\tignored\n").unwrap();
    assert_eq!(p.byte(Byte::Debug8b23c5).unwrap(), 1);
    assert_eq!(p.byte(Byte::Test8b23bc).unwrap(), 1);
    assert_eq!(p.with(|r| Ok(r.options.test_b)).unwrap(), 1);
    let retained = p.clone();
    assert!(Arc::ptr_eq(&p.0, &retained.0));
    p.world_controller_reset4c2b80().unwrap();
    p.world_reset_post_camera4b9eb1().unwrap();
    assert_eq!(retained.byte(Byte::Debug8b23c5).unwrap(), 1);
    assert_eq!(retained.byte(Byte::Test8b23bc).unwrap(), 1);
    assert!(startup(&loader(), "HOST").is_err());
    let (p, _) = startup(&loader(), "DEBUGSTUFF\0TESTMODE").unwrap();
    assert_eq!(p.byte(Byte::Test8b23bc).unwrap(), 0);
}
#[test]
fn actual_loader_rejects_missing_and_nonzero_preimages() {
    assert!(startup(&[], "").is_err());
    let mut cells = Cells::process_allocation(&loader()).unwrap();
    cells.set_byte(0x8fca96, 7).unwrap();
    assert!(startup_from_loader(&cells, "").is_err());
}
#[test]
fn count_wraps_once_and_only_recovered_camera_reset_resets_it() {
    let p = fixture();
    p.with(|r| {
        r.count = u32::MAX;
        Ok(())
    })
    .unwrap();
    p.increment_count4ba580().unwrap();
    assert_eq!(p.count822d38().unwrap(), 0);
    p.increment_count4ba580().unwrap();
    assert_eq!(p.count822d38().unwrap(), 1);
    p.world_controller_reset4c2b80().unwrap();
    assert_eq!(p.count822d38().unwrap(), 1);
    p.world_reset_post_camera4b9eb1().unwrap();
    assert_eq!(p.count822d38().unwrap(), 0);
}
#[test]
fn production_picking_writer_preserves_common_camera_block_building_domains() {
    let p = fixture();
    p.picking_write(PickingWrite::DisableAll).unwrap();
    assert_eq!(p.picking_projection().unwrap().common, 0);
    assert_eq!(p.picking_projection().unwrap().camera, 1);
    p.picking_write(PickingWrite::CameraDisable).unwrap();
    assert_eq!(p.picking_projection().unwrap().camera, 0);
    p.picking_write(PickingWrite::Block(Some(-2))).unwrap();
    let x = p.picking_projection().unwrap();
    assert_eq!((x.common, x.block, x.block_id, x.building), (1, 1, -2, 0));
    p.picking_write(PickingWrite::Building(None)).unwrap();
    let x = p.picking_projection().unwrap();
    assert_eq!(
        (x.common, x.block, x.block_id, x.building, x.building_id),
        (1, 0, -2, 1, -1)
    );
    p.picking_write(PickingWrite::DisableAll).unwrap();
    p.picking_write(PickingWrite::EnableAll).unwrap();
    assert_eq!(p.byte(Byte::Controller920888).unwrap(), 1);
}
#[test]
fn network_teardown_does_not_rederive_city_entry_b9() {
    let p = fixture();
    //Native connected constructor completion is not an admitted production
    //network transport. Exercise the separated city-entry writer in this unit
    //fixture, then the actual admitted teardown writer.
    p.with(|r| {
        r.network.enabled = 1;
        r.city_main_entry();
        Ok(())
    })
    .unwrap();
    assert_eq!(p.byte(Byte::QuitOnEscape8b23b9).unwrap(), 1);
    p.network_teardown4e1c50().unwrap();
    assert_eq!(p.byte(Byte::Network93bbb9).unwrap(), 0);
    assert_eq!(p.byte(Byte::QuitOnEscape8b23b9).unwrap(), 1);
}
#[test]
fn failed_media_probe_commits_clear_without_mutex_loan_or_success() {
    let p = fixture();
    p.validate_media(MediaSite::Startup4b94eb, || {
        assert!(p.mutex_available());
        Ok(MediaOutcome::OriginalBypass)
    })
    .unwrap();
    assert_eq!(p.byte(Byte::Media8fca98).unwrap(), 1);
    let error = p
        .validate_media(MediaSite::Periodic4b921d, || {
            assert!(p.mutex_available());
            assert_eq!(p.with(|r| Ok(r.media.byte)).unwrap(), 0);
            Err(Error::MediaProbeUnavailable)
        })
        .unwrap_err();
    assert_eq!(error, Error::MediaProbeUnavailable);
    assert_eq!(p.byte(Byte::Media8fca98), Err(error));
}
#[test]
fn sequence_producer_absent_and_reached_callback_fail_without_result_alias() {
    let p = fixture();
    assert_eq!(
        p.reached_sequence4b1980(),
        Err(Error::SequenceCallbackUnavailable)
    );
    p.with(|r| {
        r.sequence = None;
        Ok(())
    })
    .unwrap();
    assert_eq!(p.byte(Byte::Suppressed8b0700), Err(Error::UnbornSequence));
}
#[test]
fn activity_has_one_ui_owner_survives_world_reset_and_stores_in_source_order() {
    let (p, mut activity) = startup(&loader(), "").unwrap();
    activity.store_a4(1);
    assert_eq!((activity.presentation4c2bdc(), activity.a8()), (1, 0));
    activity.store_a8(1);
    p.world_controller_reset4c2b80().unwrap();
    p.world_reset_post_camera4b9eb1().unwrap();
    let retained = activity;
    assert_eq!((retained.a4(), retained.a8()), (1, 1));
}
#[test]
fn quit_is_retained_on_failure_and_is_not_tail_ack() {
    let p = fixture();
    let token = p.begin_central(CentralIdentity::new(1, 1)).unwrap();
    let old = token.0;
    p.request_quit(QuitWriter::Central).unwrap();
    assert_eq!(p.quit_requested().unwrap(), 1);
    assert!(!p.exit_ready().unwrap());
    p.fail_central(token).unwrap();
    assert!(!p.exit_ready().unwrap());
    p.request_quit(QuitWriter::Window).unwrap();
    assert!(!p.exit_ready().unwrap());
    let next = p.begin_central(CentralIdentity::new(2, 1)).unwrap();
    p.complete_central_tail(next).unwrap();
    assert!(
        !p.exit_ready().unwrap(),
        "new Host/operation cannot discharge failed prior quit"
    );
    assert_eq!(
        p.complete_central_tail(CentralToken(old)),
        Err(Error::CentralTokenMismatch)
    );
    let p = fixture();
    p.request_quit(QuitWriter::Menu).unwrap();
    assert!(p.exit_ready().unwrap());
}
#[test]
fn only_matching_active_operation_completes_its_quit_tail() {
    let p = fixture();
    assert_eq!(
        p.request_quit(QuitWriter::Central),
        Err(Error::NoActiveCentral)
    );
    let token = p.begin_central(CentralIdentity::new(3, 7)).unwrap();
    let wrong = CentralToken(CentralId {
        serial: token.0.serial,
        operation: CentralIdentity::new(3, 8),
    });
    p.request_quit(QuitWriter::Central).unwrap();
    assert_eq!(
        p.complete_central_tail(wrong),
        Err(Error::CentralTokenMismatch)
    );
    assert!(!p.exit_ready().unwrap());
    p.complete_central_tail(token).unwrap();
    assert!(p.exit_ready().unwrap());
}
#[test]
fn duplicate_begin_and_internal_lease_exhaustion_do_not_authorize_or_reuse_work() {
    let p = fixture();
    let token = p.begin_central(CentralIdentity::new(4, 2)).unwrap();
    assert_eq!(
        p.begin_central(CentralIdentity::new(4, 3)).err(),
        Some(Error::CentralAlreadyActive)
    );
    p.fail_central(token).unwrap();
    p.with(|r| {
        r.quit.next_serial = u64::MAX;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        p.begin_central(CentralIdentity::new(4, 3)).err(),
        Some(Error::CentralTokenExhausted)
    );
    assert_eq!(p.quit_requested().unwrap(), 0);
    assert!(!p.exit_ready().unwrap());
}

#[test]
fn legacy_saved_scene_remains_unavailable_instead_of_mode_inference() {
    let p = fixture();
    p.source_load_unjoined().unwrap();
    assert_eq!(p.scene_available(), Err(Error::SourceLoadOrderUnavailable));
    p.source_timing_mode_unjoined().unwrap();
    assert_eq!(
        p.scene_available(),
        Err(Error::TimingModeCallbackUnavailable)
    );
    p.source_fresh_world().unwrap();
    p.scene_available().unwrap();
}

#[test]
fn performance_constructor_graphics_writers_and_world_lifetimes_are_distinct() {
    let p = fixture();
    let retained = p.clone();
    assert_eq!(p.performance8b26cc().unwrap(), 2);
    assert_eq!(
        p.performance_producer().unwrap(),
        PerformanceProducer::Constructor4b91c3
    );
    p.set_performance8b26cc(1).unwrap();
    p.graphics_construction4c6810(OriginalGraphicsPolicy::NormalD3d)
        .unwrap();
    assert_eq!(
        retained.performance8b26cc().unwrap(),
        1,
        "D3D retains a prior Lua integer"
    );
    p.graphics_construction4c6810(OriginalGraphicsPolicy::SoftwareKind2Or3)
        .unwrap();
    assert_eq!(retained.performance8b26cc().unwrap(), 0);
    assert_eq!(
        p.performance_producer().unwrap(),
        PerformanceProducer::Software4c684a
    );
    p.graphics_construction4c6810(OriginalGraphicsPolicy::NormalD3d)
        .unwrap();
    assert_eq!(
        p.performance8b26cc().unwrap(),
        0,
        "D3D does not reinitialize after Software/Test"
    );
    p.set_performance8b26cc(-7).unwrap();
    p.world_reset_post_camera4b9eb1().unwrap();
    p.world_controller_reset4c2b80().unwrap();
    p.source_fresh_world().unwrap();
    assert_eq!(
        retained.performance8b26cc().unwrap(),
        -7,
        "World reset is not process birth"
    );
    assert!(
        retained.performance_nonzero().unwrap(),
        "camera uses signed-word nonzero, not quality enum"
    );
    assert_eq!(
        p.performance_producer().unwrap(),
        PerformanceProducer::Lua4d6980
    );
    let mut cells = Cells::process_allocation(&loader()).unwrap();
    cells.set_word(0x8b26cc, 1).unwrap();
    assert!(
        startup_from_loader(&cells, "").is_err(),
        "verify loader preimage before constructor store"
    );
}
#[test]
fn performance_poison_is_an_error_not_a_high_default() {
    let p = fixture();
    let other = p.clone();
    let _ = std::thread::spawn(move || {
        let _guard = other.0.lock().unwrap();
        panic!("intentional process poison");
    })
    .join();
    assert_eq!(p.performance8b26cc(), Err(Error::Poisoned));
    assert_eq!(p.set_performance8b26cc(2), Err(Error::Poisoned));
    assert_eq!(p.performance_nonzero(), Err(Error::Poisoned));
}

#[test]
fn ordinary_frontend_receipt_is_single_use_and_rejects_changed_loader_mode() {
    let (_, mut activity) = startup(&loader(), "").unwrap();
    let receipt = activity.take_frontend_startup().unwrap();
    assert_eq!(
        (
            receipt.disable_sound,
            receipt.disable_music,
            receipt.sound,
            receipt.suspended
        ),
        (0, 0, 1, 0)
    );
    assert_eq!(receipt.input.input_enabled, 1);
    assert_eq!(receipt.pause.bytes, [0; 9]);
    assert!(activity.take_frontend_startup().is_err());
    let mut cells = Cells::process_allocation(&loader()).unwrap();
    cells.set_byte(0x8af23b, 1).unwrap();
    assert!(startup_from_loader(&cells, "").is_err());
}
