//! Prepared source regressions. No GPU/original-resource fixture is needed.
use super::*;
use crate::locator::strategic::{
    self, EnabledBufferProvenance, NormalAllocationPath, NormalLiveEnabledBuffer,
};
use openmt2001::{
    camera::bank::{Bank, Capture},
    render_scene::{InitializedApartmentInspection, InspectionBuilding},
};
use std::sync::{Arc, mpsc};

struct Headless {
    locator: Option<Locator>,
    adjustment: Option<business_adjustment::State>,
    graph: Option<camera_live::ui_graph::UiGraphState>,
    ui: Option<camera_live::State>,
    navigation: Option<camera_live::navigation::State>,
    focus: camera_live::focus::Focus,
    endpoint: Option<camera_live::endpoint::Endpoint>,
    click: bool,
    tooltip: Option<openmt2001::render::PreparedScene>,
    tooltip_state: Option<(u32, [i32; 2])>,
    icon_tooltip_dwell: crate::tooltip::IconDwell,
    // Fixture's process-lifetime resource, separate from retained World owners.
    pavement: Option<crate::selection::Assets>,
}
impl Headless {
    fn empty() -> Self {
        Self {
            locator: Some(crate::locator::refresh_ui_tests::fresh()),
            adjustment: None,
            graph: None,
            ui: None,
            navigation: None,
            focus: Default::default(),
            endpoint: None,
            click: false,
            tooltip: None,
            tooltip_state: None,
            icon_tooltip_dwell: Default::default(),
            pavement: None,
        }
    }
    fn owners(&mut self) -> Owners<'_> {
        Owners {
            locator: &mut self.locator,
            adjustment: &mut self.adjustment,
            graph: &mut self.graph,
            ui: &mut self.ui,
            navigation: &mut self.navigation,
            focus: &mut self.focus,
            endpoint: &mut self.endpoint,
            navigation_click_latched: &mut self.click,
            tooltip: &mut self.tooltip,
            tooltip_state: &mut self.tooltip_state,
            icon_tooltip_dwell: &mut self.icon_tooltip_dwell,
        }
    }
}
fn scene(buildings: &[InspectionBuilding]) -> RefreshScene<'_> {
    RefreshScene {
        buildings,
        catalog: None,
        commodities: None,
    }
}
fn apartment(slot: usize, rent: f32) -> InspectionBuilding {
    let mut record = [0; 544];
    record[0] = 1;
    record[10] = 1;
    record[0x21c] = 1;
    record[0xe..0x10].copy_from_slice(&10u16.to_le_bytes());
    record[0x204..0x208].copy_from_slice(&rent.to_le_bytes());
    InspectionBuilding {
        category: 3,
        saved_slot: None,
        runtime_slot: Some(slot),
        lot: [0; 4],
        height: 1,
        kind: 1,
        quality: 0,
        saved_business: None,
        initialized_business: None,
        initialized_apartment: Some(InitializedApartmentInspection {
            slot: 4,
            name_text_id: 7,
            record,
            owner_character: None,
        }),
        saved_owner_character: None,
        camera_entrance: None,
        bounds: [[0.; 3]; 8],
        center: [0.; 3],
        radius: 1.,
        translation: [0.; 3],
    }
}
fn witness() -> EnabledBufferProvenance {
    EnabledBufferProvenance::NormalLive(
        NormalLiveEnabledBuffer::from_live_unadjusted_684_allocation(
            0x940a38,
            4,
            16,
            NormalAllocationPath::ReusedWhole,
        )
        .unwrap(),
    )
}
fn retained(
    graph: bool,
    buildings: &[InspectionBuilding],
    assets: &business_adjustment::Assets,
) -> (
    Headless,
    crate::live_host::camera_bridge::Worker,
    mpsc::Sender<Result<[u8; 544], String>>,
) {
    let mut h = Headless::empty();
    let mut c = strategic::selector_boundary_tests::state_fixture(
        crate::compatibility::Mode::StrictV14,
        witness(),
    );
    c.tab = 2;
    c.cache = 0x1234;
    c.poll.counters[3] = 17;
    c.dirty_writeback = true;
    Arc::make_mut(&mut c.input).owned.push(strategic::Owned {
        category: 3,
        label: 4,
        building_slot: 8,
        building_reference: 0x591318 + 8 * 916,
    });
    h.locator.as_mut().unwrap().install_strategic(c).unwrap();
    let mut colors = h
        .locator
        .as_ref()
        .unwrap()
        .strategic()
        .unwrap()
        .input
        .before_colors;
    colors[0] = 0xff345678;
    if graph {
        h.graph = Some(camera_live::ui_graph::refresh_fixture(colors));
    }
    h.adjustment = assets
        .open_for_owner_view(scene(buildings), 0, Some(1))
        .unwrap();
    let (edit, answer) = mpsc::channel();
    h.adjustment.as_mut().unwrap().edit_request = None;
    h.adjustment.as_mut().unwrap().edit_pending = Some(answer);
    h.adjustment.as_mut().unwrap().edit_inflight = Some(crate::business_edit::Action::Refresh);
    h.ui = Some(camera_live::State::new(
        camera_live::Globals::after_normal_initialization(
            camera_live::OriginalGraphicsPolicy::NormalD3d,
            false,
        ),
        camera_live::Cursor::new(2, 19).unwrap(),
        None,
        None,
    ));
    h.navigation = Some(camera_live::navigation::State::new(
        3,
        4,
        2,
        [1, 0, 1],
        None,
    ));
    h.focus
        .show_child(camera_live::focus::ChildKey::Compass, 1143, true)
        .unwrap();
    h.click = true;
    let pavement = crate::selection::Assets::descriptor_test_assets(vec![], [0; 116]);
    let (endpoint, worker) = camera_live::endpoint::source_lifecycle_tests::install(198, &pavement);
    h.endpoint = Some(endpoint);
    h.pavement = Some(pavement);
    (h, worker, edit)
}
struct Pins {
    node: *const strategic::tree::Node,
    input: Arc<strategic::Inputs>,
    capture: Capture,
    f_input: Option<Arc<crate::navigator::upper::Inputs>>,
    property: camera_live::property_descriptors::Handle,
    pool: (u64, u64),
    dirty: u8,
}
impl Pins {
    fn new(h: &Headless) -> Self {
        let s = h.locator.as_ref().unwrap().strategic().unwrap();
        Self {
            node: s.tree.nodes.get(&5).unwrap() as *const _,
            input: s.input.clone(),
            capture: h.endpoint.as_ref().unwrap().capture(),
            property: h
                .endpoint
                .as_ref()
                .unwrap()
                .owner()
                .property_descriptors()
                .unwrap()
                .lookup(0)
                .unwrap()
                .unwrap(),
            pool: h
                .endpoint
                .as_ref()
                .unwrap()
                .owner()
                .property_descriptors()
                .unwrap()
                .test_identity(),
            dirty: h.endpoint.as_ref().unwrap().owner().kind_dirty(),
            f_input: h
                .graph
                .as_ref()
                .and_then(|g| g.upper.as_ref().map(|f| f.input.clone())),
        }
    }
    fn assert(&self, h: &Headless) {
        let s = h.locator.as_ref().unwrap().strategic().unwrap();
        assert_eq!(s.tree.nodes.get(&5).unwrap() as *const _, self.node);
        assert!(Arc::ptr_eq(&self.input, &s.input));
        assert_eq!(s.cache, 0x1234);
        assert_eq!(
            (s.selected_business_row, s.selected_commodity_row),
            (Some(2), Some(1))
        );
        assert_eq!((s.world_business, s.world_commodity), (42, 19));
        assert_eq!(s.poll.counters[3], 17);
        assert!(s.dirty_writeback);
        assert_eq!(s.commodity_enabled_provenance, witness());
        assert_eq!(h.endpoint.as_ref().unwrap().capture(), self.capture);
        assert_eq!(
            h.endpoint.as_ref().unwrap().owner().kind_dirty(),
            self.dirty
        );
        let pool = h
            .endpoint
            .as_ref()
            .unwrap()
            .owner()
            .property_descriptors()
            .unwrap();
        assert_eq!(pool.test_identity(), self.pool);
        assert_eq!(pool.lookup(0).unwrap(), Some(self.property));
        assert!(pool.resolve(self.property).is_ok());
        assert_eq!(h.ui.as_ref().unwrap().cursor_index(), 1);
        assert_eq!(h.navigation.as_ref().unwrap().mode(), 3);
        assert_eq!(h.navigation.as_ref().unwrap().saved_visibility(), [1, 0, 1]);
        assert_eq!(h.focus.caption(), Some(1143));
        assert!(h.click);
        assert!(h.adjustment.as_ref().unwrap().edit_pending.is_some());
        if let Some(input) = &self.f_input {
            let g = h.graph.as_ref().unwrap();
            let f = g.upper.as_ref().unwrap();
            assert!(Arc::ptr_eq(input, &f.input));
            assert_eq!((f.selected, f.active, f.enabled), (7, 1, 1));
            assert_eq!(g.world.colors[0], 0xff345678);
            assert_eq!(g.world.messages, 17);
            assert!(g.groups[0].selected.is_some());
        }
    }
}
fn prepare_refresh(
    old: &mut Headless,
    next: &mut Headless,
    old_buildings: &[InspectionBuilding],
    new_buildings: &[InspectionBuilding],
    assets: &business_adjustment::Assets,
    retry: &mut Retry,
) -> Result<Prepared, String> {
    prepare(
        &old.owners(),
        &mut next.owners(),
        Inputs {
            previous_buildings: old_buildings,
            next_scene: scene(new_buildings),
            locator_extents: [19, 17],
            business_owner: Some(1),
        },
        assets,
        retry,
    )
}
fn complete_retry(
    mut old: Headless,
    mut next: Headless,
    old_buildings: &[InspectionBuilding],
    new_buildings: &[InspectionBuilding],
    assets: &business_adjustment::Assets,
    retry: &mut Retry,
    pins: Pins,
    worker: crate::live_host::camera_bridge::Worker,
    edit: mpsc::Sender<Result<[u8; 544], String>>,
) {
    assert!(retry.error.is_some());
    assert!(retry.due(Instant::now() + Duration::from_secs(2)));
    // Use the production retry sender and its actual Inspect request type.
    let (sender, requests) = mpsc::channel();
    let camera = super::super::fresh_city_camera().0;
    let _response = retry.request(&sender, camera).unwrap();
    let request = requests.try_recv().unwrap();
    let super::super::FreshRequest::Inspect {
        camera: requested,
        reply: _reply,
    } = request
    else {
        panic!("retry must send the actual Inspect request")
    };
    assert_eq!(requested.eye, camera.eye);
    assert!(!retry.due(Instant::now() + Duration::from_secs(2)));
    let plan = prepare_refresh(
        &mut old,
        &mut next,
        old_buildings,
        new_buildings,
        assets,
        retry,
    )
    .unwrap();
    // Preparation still has not consumed the protected owner set.
    pins.assert(&old);
    assert!(!next.locator.as_ref().unwrap().has_strategic());
    commit(&mut old.owners(), &mut next.owners(), plan, retry);
    pins.assert(&next);
    assert!(!worker.retired());
    assert!(
        old.endpoint.is_none()
            && old.ui.is_none()
            && old.graph.is_none()
            && old.adjustment.is_none()
    );
    assert!(!old.locator.as_ref().unwrap().has_strategic());
    assert!(retry.error.is_none());
    assert!(!retry.due(Instant::now() + Duration::from_secs(2)));
    // The actual pending edit channel, rather than a reconstructed State,
    // survives and remains serviceable after publication.
    edit.send(Ok([9; 544])).unwrap();
    assert_eq!(
        next.adjustment
            .as_ref()
            .unwrap()
            .edit_pending
            .as_ref()
            .unwrap()
            .try_recv()
            .unwrap()
            .unwrap(),
        [9; 544]
    );
    assert_eq!(next.adjustment.as_ref().unwrap().building, 1);
    // Reach the SAME production protected supplier-owner boundary. Pending
    // dirty poll/color effects must reject replacement until extracted.
    let locator = next.locator.as_mut().unwrap();
    let mut colors = next
        .graph
        .as_ref()
        .map_or(pins.input.before_colors, |g| g.world.colors);
    assert!(
        locator
            .strategic()
            .unwrap()
            .check_suppliers(&pins.input, &colors)
            .is_err()
    );
    let write = locator.take_strategic_writeback(colors).unwrap().unwrap();
    assert_eq!(write.poll.counters[3], 17);
    assert_eq!(write.before_poll, pins.input.poll);
    assert_eq!(write.colors, colors);
    let input = locator.strategic().unwrap().input.clone();
    if let Some(g) = &next.graph {
        camera_live::strategic_panel::check_supplier_owner(
            Some(locator),
            &colors,
            g.panels
                .get(&camera_live::ui_graph::PanelId::C)
                .map(|p| p.children.as_slice()),
            Some(&input),
            None,
        )
        .unwrap();
    }
    camera_live::strategic_panel::replace_supplier_owner(locator, &mut colors, input.clone())
        .unwrap();
    assert_eq!(
        locator.strategic().unwrap().tree.nodes.get(&5).unwrap() as *const _,
        pins.node
    );
    assert_eq!(
        locator.strategic().unwrap().commodity_enabled_provenance,
        witness()
    );
    // Explicitly retire this same Inbox generation only after the assertions.
    let owner = next.endpoint.take().unwrap().into_owner();
    assert_eq!(owner.capture(), pins.capture);
    assert!(worker.retired());
}
#[test]
fn intro_refresh_c_preflight_rejection_keeps_actual_owners_and_retries() {
    let assets = business_adjustment::refresh_assets_fixture();
    let old_buildings = vec![apartment(8, 10.)];
    let (mut old, worker, edit) = retained(true, &old_buildings, &assets);
    let pins = Pins::new(&old);
    let mut next = Headless::empty();
    let mut retry = Retry::default();
    let error = prepare_refresh(
        &mut old,
        &mut next,
        &old_buildings,
        &[],
        &assets,
        &mut retry,
    )
    .err()
    .unwrap();
    assert_eq!(
        error,
        "C stable owned building absent from refreshed display projection"
    );
    pins.assert(&old);
    assert!(!worker.retired());
    assert!(!next.locator.as_ref().unwrap().has_strategic());
    let new_buildings = vec![apartment(9, 2.), apartment(8, 12.)];
    complete_retry(
        old,
        next,
        &old_buildings,
        &new_buildings,
        &assets,
        &mut retry,
        pins,
        worker,
        edit,
    );
}
#[test]
fn intro_refresh_late_adjustment_error_precedes_every_take_and_retries() {
    let assets = business_adjustment::refresh_assets_fixture();
    let old_buildings = vec![apartment(8, 10.)];
    let (mut old, worker, edit) = retained(true, &old_buildings, &assets);
    let pins = Pins::new(&old);
    let mut next = Headless::empty();
    let mut retry = Retry::default();
    let bad = vec![apartment(9, 2.), apartment(8, f32::NAN)];
    let error = prepare_refresh(
        &mut old,
        &mut next,
        &old_buildings,
        &bad,
        &assets,
        &mut retry,
    )
    .err()
    .unwrap();
    assert_eq!(error, "nonfinite apartment display value");
    pins.assert(&old);
    assert!(!worker.retired());
    assert!(!next.locator.as_ref().unwrap().has_strategic());
    let good = vec![apartment(9, 2.), apartment(8, 12.)];
    complete_retry(
        old,
        next,
        &old_buildings,
        &good,
        &assets,
        &mut retry,
        pins,
        worker,
        edit,
    );
}
#[test]
fn intro_refresh_component_graph_none_keeps_c_and_endpoint_on_late_error() {
    let assets = business_adjustment::refresh_assets_fixture();
    let old_buildings = vec![apartment(8, 10.)];
    let (mut old, worker, edit) = retained(false, &old_buildings, &assets);
    let pins = Pins::new(&old);
    let mut next = Headless::empty();
    let mut retry = Retry::default();
    let bad = vec![apartment(9, 2.), apartment(8, f32::MAX)];
    assert_eq!(
        prepare_refresh(
            &mut old,
            &mut next,
            &old_buildings,
            &bad,
            &assets,
            &mut retry
        )
        .err()
        .unwrap(),
        "apartment potential rent display range"
    );
    pins.assert(&old);
    assert!(!worker.retired());
    let good = vec![apartment(9, 2.), apartment(8, 12.)];
    complete_retry(
        old,
        next,
        &old_buildings,
        &good,
        &assets,
        &mut retry,
        pins,
        worker,
        edit,
    );
}
#[test]
fn intro_refresh_legacy_financial_projection_rejection_is_local_and_retryable() {
    let assets = business_adjustment::refresh_assets_fixture();
    let mut old = Headless::empty();
    let mut next = Headless::empty();
    let mut ledger = [0; 440];
    ledger[0xc] = 1;
    old.locator
        .as_mut()
        .unwrap()
        .attach_players(crate::player_info::State::new(&[ledger]).unwrap());
    let mut players = crate::player_info::State::new(&[ledger]).unwrap();
    // Corrupt the prospective report after attachment, at the actual legacy
    // retain boundary. No renderer or substitute callback handles the error.
    players.records[0][0x140] = 2;
    next.locator.as_mut().unwrap().attach_players(players);
    let (worker, inbox) = crate::live_host::camera_bridge::channel(199);
    old.endpoint = Some(camera_live::endpoint::Endpoint::new(
        camera_live::owner::Owner::new(Bank::loader_zero_storage(), 1),
        inbox,
    ));
    let mut retry = Retry::default();
    assert_eq!(
        prepare_refresh(&mut old, &mut next, &[], &[], &assets, &mut retry)
            .err()
            .unwrap(),
        "saved player ledger selector leaves banks"
    );
    assert!(old.endpoint.is_some());
    assert!(!worker.retired());
    assert!(retry.error.is_some());
    next.locator
        .as_mut()
        .unwrap()
        .attach_players(crate::player_info::State::new(&[ledger]).unwrap());
    let plan = prepare_refresh(&mut old, &mut next, &[], &[], &assets, &mut retry).unwrap();
    commit(&mut old.owners(), &mut next.owners(), plan, &mut retry);
    assert!(old.endpoint.is_none() && next.endpoint.is_some());
    assert!(next.graph.is_none());
    assert!(retry.error.is_none());
    assert!(!worker.retired());
    next.endpoint.take().unwrap().into_owner();
    assert!(worker.retired());
}

#[test]
fn intro_refresh_retry_send_error_keeps_request_pending_without_owner_consumption() {
    let (sender, requests) = mpsc::channel();
    drop(requests);
    let mut retry = Retry::default();
    retry.reject("preparation rejected".into());
    assert!(
        retry
            .request(&sender, super::super::fresh_city_camera().0)
            .is_err()
    );
    assert!(retry.error.is_some());
    assert!(retry.due(Instant::now() + Duration::from_secs(2)));
}

#[test]
fn options_retirement_cancels_pending_refresh_reply_before_it_can_reject_or_retry() {
    let assets = business_adjustment::refresh_assets_fixture();
    let buildings = vec![apartment(8, 12.)];
    let (mut old, worker, _edit) = retained(true, &buildings, &assets);
    let raw = old.endpoint.as_ref().unwrap().capture();
    let dirty = old.endpoint.as_ref().unwrap().owner().kind_dirty();
    let (late, answer) =
        mpsc::channel::<Result<super::super::LoadedInspection, crate::load::Failure>>();
    let mut loading = Some(answer);
    let mut refreshing = true;
    let mut parked = None;
    let mut retry = Retry::default();
    retry.reject("earlier refresh rejected".into());
    retire(
        &mut retry,
        &mut refreshing,
        &mut loading,
        Some(&mut old.endpoint),
        &mut parked,
    )
    .unwrap();
    assert!(!refreshing && loading.is_none());
    assert!(old.endpoint.is_none());
    assert!(retry.error.is_none());
    assert!(!retry.due(Instant::now() + Duration::from_secs(2)));
    assert!(
        late.send(Err(
            "late inspect error must be discarded after Options Quit".into()
        ))
        .is_err()
    );
    assert!(retry.error.is_none());
    assert!(worker.retired());
    assert_eq!(parked.as_ref().unwrap().capture(), raw);
    assert_eq!(parked.as_ref().unwrap().kind_dirty(), dirty);
}
#[test]
fn retirement_preserves_an_independent_new_load_receiver() {
    let (sender, answer) =
        mpsc::channel::<Result<super::super::LoadedInspection, crate::load::Failure>>();
    let mut loading = Some(answer);
    let mut refreshing = false;
    let mut parked = None;
    let mut retry = Retry::default();
    retry.reject("old refresh state".into());
    retire(&mut retry, &mut refreshing, &mut loading, None, &mut parked).unwrap();
    assert!(loading.is_some() && !refreshing);
    assert!(retry.error.is_none());
    sender
        .send(Err("independent initial load remains owned".into()))
        .unwrap();
    assert_eq!(
        loading.as_ref().unwrap().try_recv().unwrap().err().unwrap(),
        "independent initial load remains owned"
    );
}

#[test]
fn retirement_helper_parks_same_retired_pool_and_reports_exhaustion() {
    let assets = business_adjustment::refresh_assets_fixture();
    let buildings = vec![apartment(8, 12.)];
    let (mut old, worker, _edit) = retained(true, &buildings, &assets);
    let endpoint = old.endpoint.as_mut().unwrap();
    let identity = endpoint
        .owner()
        .property_descriptors()
        .unwrap()
        .test_identity();
    let handle = endpoint
        .owner()
        .property_descriptors()
        .unwrap()
        .lookup(0)
        .unwrap()
        .unwrap();
    // Exhaust the actual retained allocation counter through a test-only seam.
    endpoint.test_exhaust_property_generation();
    let raw = endpoint.capture();
    let dirty = endpoint.owner().kind_dirty();
    let mut retry = Retry::default();
    let mut refreshing = false;
    let mut loading = None;
    let mut parked = None;
    assert_eq!(
        retire(
            &mut retry,
            &mut refreshing,
            &mut loading,
            Some(&mut old.endpoint),
            &mut parked
        ),
        Err("descriptor generation exhausted".into())
    );
    assert!(old.endpoint.is_none());
    assert!(worker.retired());
    let owner = parked.as_ref().unwrap();
    assert_eq!(owner.capture(), raw);
    assert_eq!(owner.kind_dirty(), dirty);
    assert_eq!(owner.test_retained_pool_identity().unwrap().0, identity.0);
    assert!(owner.test_retained_pool_is_retired());
    assert_eq!(
        owner.property_lifecycle_error(),
        Some("descriptor generation exhausted")
    );
    assert!(owner.property_descriptors().is_err());
    // The old handle is retained as evidence; no category-based rebind occurs.
    let _retained_stale_alias = handle;
}

#[test]
fn actual_refresh_commit_retains_same_resident_registry_until_real_retirement() {
    let assets = business_adjustment::refresh_assets_fixture();
    let pavement = crate::selection::Assets::descriptor_test_assets(vec![], [0; 116]);
    let (endpoint, worker, pool) =
        camera_live::endpoint::resident_lifecycle_tests::installed_for_refresh(960, &pavement);
    let registry = endpoint.owner().resident_descriptors().unwrap().identity();
    let selected = endpoint.owner().current_ui_selected().unwrap();
    let source = endpoint
        .owner()
        .resident_descriptors()
        .unwrap()
        .source()
        .unwrap()
        .clone();
    let mut old = Headless::empty();
    old.endpoint = Some(endpoint);
    old.pavement = Some(pavement);
    let mut next = Headless::empty();
    let mut retry = Retry::default();
    let plan = prepare_refresh(&mut old, &mut next, &[], &[], &assets, &mut retry).unwrap();
    assert_eq!(
        old.endpoint
            .as_ref()
            .unwrap()
            .owner()
            .resident_descriptors()
            .unwrap()
            .identity(),
        registry
    );
    commit(&mut old.owners(), &mut next.owners(), plan, &mut retry);
    assert!(old.endpoint.is_none());
    assert!(!worker.retired());
    let owner = next.endpoint.as_ref().unwrap().owner();
    assert_eq!(owner.resident_descriptors().unwrap().identity(), registry);
    assert_eq!(
        owner.resident_descriptors().unwrap().source().unwrap(),
        &source
    );
    assert_eq!(owner.current_ui_selected().unwrap(), selected);
    assert_eq!(owner.published_ui_selected().unwrap(), selected);
    assert_eq!(owner.transient_ui_selected().unwrap(), selected);
    // Actual outer retirement retires registry independently of pool drop.
    let mut refreshing = false;
    let mut loading = None;
    let mut parked = None;
    retire(
        &mut retry,
        &mut refreshing,
        &mut loading,
        Some(&mut next.endpoint),
        &mut parked,
    )
    .unwrap();
    assert!(worker.retired());
    assert!(pool.source().validate_live().is_ok());
    assert!(
        parked
            .as_ref()
            .unwrap()
            .resident_descriptors()
            .unwrap()
            .source()
            .is_err()
    );
    assert!(parked.as_ref().unwrap().current_ui_selected().is_err());
}

#[test]
fn tooltip_dwell_and_visible_draw_survive_repeated_scene_refreshes() {
    // Toolbar, sidebar and locator identifiers use the same retained timer.
    for id in [29, 1141, 1146] {
        let assets = business_adjustment::refresh_assets_fixture();
        let mut retry = Retry::default();
        let mut view = Headless::empty();
        let mut baseline = crate::tooltip::IconDwell::default();
        for frame in 1..=30 {
            if [8, 17, 28].contains(&frame) {
                let mut next = Headless::empty();
                let plan =
                    prepare_refresh(&mut view, &mut next, &[], &[], &assets, &mut retry).unwrap();
                let was_visible = view.tooltip.is_some();
                let previous_caption = view.tooltip_state;
                commit(&mut view.owners(), &mut next.owners(), plan, &mut retry);
                // The third refresh happens after paint: retain both the
                // prepared draw and its cache key without waiting for a draw.
                assert_eq!(next.tooltip.is_some(), was_visible);
                assert_eq!(next.tooltip_state, previous_caption);
                assert!(view.tooltip.is_none());
                assert!(view.tooltip_state.is_none());
                view = next;
            }
            let visible = view.icon_tooltip_dwell.advance(Some(id));
            assert_eq!(
                visible,
                baseline.advance(Some(id)),
                "frame {frame}, ID {id}"
            );
            assert_eq!(visible, (frame >= 26).then_some(id));
            if let Some(id) = visible {
                view.tooltip = Some(Default::default());
                view.tooltip_state = Some((id, [40, 100]));
            }
        }
    }
}

#[test]
fn tooltip_dwell_resets_on_target_change_exit_and_new_city() {
    let assets = business_adjustment::refresh_assets_fixture();
    let mut retry = Retry::default();
    let mut previous = Headless::empty();
    for _ in 0..26 {
        previous.icon_tooltip_dwell.advance(Some(1146));
    }
    let mut view = Headless::empty();
    let plan = prepare_refresh(&mut previous, &mut view, &[], &[], &assets, &mut retry).unwrap();
    commit(&mut previous.owners(), &mut view.owners(), plan, &mut retry);
    for _ in 0..25 {
        assert_eq!(view.icon_tooltip_dwell.advance(Some(29)), None);
    }
    assert_eq!(view.icon_tooltip_dwell.advance(Some(29)), Some(29));
    assert_eq!(view.icon_tooltip_dwell.advance(None), None);
    for _ in 0..25 {
        assert_eq!(view.icon_tooltip_dwell.advance(Some(29)), None);
    }
    assert_eq!(view.icon_tooltip_dwell.advance(Some(29)), Some(29));

    // City entry and true Load create a fresh view without refresh::commit.
    // Even the same icon must dwell again after that boundary.
    let mut new_city = Headless::empty();
    assert!(new_city.tooltip.is_none());
    assert!(new_city.tooltip_state.is_none());
    for _ in 0..25 {
        assert_eq!(new_city.icon_tooltip_dwell.advance(Some(29)), None);
    }
    assert_eq!(new_city.icon_tooltip_dwell.advance(Some(29)), Some(29));
}
