//! Caller regressions over real Assets/Inspection/boxed process owner,
//! empty GPU products, no Host/channel worker or production callback defaults.
//! Graph/Main controls are authored finite CPU fixtures, not full48 acceptance.
use super::super::{
    bindings, case_zero,
    case_zero_text::Packet,
    group_bridge::{Bridge, MissingFamilies},
    group_dispatch::Dispatch,
    main_panel::{self, Access, ProcessTextAccess, regression_tests},
    navigation,
    other_child_resets::{Business, Currency, Five, Node},
    process_lifetime::Parked,
    ui_graph::{self, PanelId, WidgetId, WidgetKind},
};
use super::*;
use crate::intro::{Assets, Inspection, Paths};
use std::{collections::BTreeMap, path::PathBuf};

fn path(key: &str) -> PathBuf {
    openmt2001::fixtures::path(key)
}
fn assets() -> Assets {
    Assets::load(Paths {
        compatibility: crate::compatibility::Mode::StrictV14,
        installer: path("OPENMT2001_INSTALLER"),
        ffmpeg: path("OPENMT2001_FFMPEG"),
        scene: path("OPENMT2001_SCENE"),
        executable: path("OPENMT2001_EXE"),
        map: path("OPENMT2001_INTRO_MAP"),
        quality: Some("low".into()),
        language: path("OPENMT2001_LANGUAGE"),
        profile_root: openmt2001::fixtures::scratch("TEXT_PROFILE"),
        save_root: openmt2001::fixtures::scratch("TEXT_SAVES"),
    })
    .unwrap()
}
fn inspection(assets: &Assets) -> Inspection {
    let camera = assets.flyby.current();
    let mut graph = ui_graph::refresh_fixture([0; 600]);
    graph.main.case_zero.literals = assets.ui_text.literals.clone();
    //Tests do not activate G/B/H/C show, input, central transport or full48.
    //Keep E/F already shown so navigation's ordinary return does not invent
    //the missing containing F staged-input factory just to reach Main.
    for id in [PanelId::E, PanelId::F] {
        graph.panel_mut(id).unwrap().requested = 1;
    }
    Inspection {
        tutorial_start: Default::default(),
        smoke_draw: None,
        smoke_state: None,
        music_admission: None, // explicit isolated fixture, no composed focus Route
        camera_producer: None,
        camera_ready: None,
        camera_epoch: None,
        camera_turns: Default::default(),
        ui_graph: Some(graph),
        camera_navigation: Some(navigation::State::new(
            0,
            0,
            0,
            [0; 3],
            Some(ui_graph::consumers()),
        )),
        camera_focus: Default::default(),
        navigation_click_latched: Default::default(),
        camera_endpoint: None,
        camera_capture_pending: None,
        camera_ui: Some(State::new(
            Globals::after_normal_initialization(OriginalGraphicsPolicy::NormalD3d, false),
            super::super::Cursor::new(0, 0).unwrap(),
            None,
            None,
        )),
        save_profile_root: None,
        saved_camera: Default::default(),
        result_pending: None,
        result: None,
        auction_requests: None,
        auction_audio_requests: None,
        auction_audio_player: crate::live_host::auction_audio::Player::new(
            assets.auction_audio.clone(),
        ),
        transport_audio_requests: None,
        transport_audio_player: crate::transport_audio::Player::new(assets.transport_audio.clone()),
        era_audio_requests: None,
        era_audio_player: crate::era_audio::Player::new(assets.era_audio.clone()),
        gameplay_sound_requests: None,
        gameplay_sound_player: crate::gameplay_sound::Player::new(assets.gameplay_sound.clone()),
        auction: Default::default(),
        auction_panel: Default::default(),
        notifications: Default::default(),
        planning: None,
        planning_pending: None,
        planning_pressed: Default::default(),
        live_status: None,
        time_panel: Default::default(),
        cash_arrow: Default::default(),
        dust: Default::default(),
        time_pressed: None,
        game_options: Default::default(),
        save_dialog: Default::default(),
        save_pending: None,
        options_ack: Default::default(),
        save_delete_pending: None,
        settings_menu: Default::default(),
        pulse_draw: None,
        pulse_state: None,
        status_pending: None,
        environment_pending: None,
        environment_tick: Default::default(),
        environment_time: None,
        last_status_query: std::time::Instant::now(),
        last_snapshot: std::time::Instant::now(),
        construction_active: Default::default(),
        snapshot_tick: Default::default(),
        card_input: Default::default(),
        card_selection: None,
        prompts: Default::default(),
        event_list: Default::default(),
        stats_panel: Default::default(),
        market_share_panel: Default::default(),
        stats_requested: Default::default(),
        tutorial_camera_seen: Default::default(),
        auction_notice: Default::default(),
        prompt_reply: None,
        block_information: None,
        utility_report: Default::default(),
        property_pending: None,
        rights_pending: None,
        comparison: None,
        chat_panel: crate::chat_panel::Panel::default(),
        trees: None,
        people: None,
        people_pending: None,
        last_people_query: std::time::Instant::now(),
        vehicles: None,
        vehicles_pending: None,
        transport: None,
        transport_pending: None,
        _fresh_session: None,
        adjustment: None,
        operations: Default::default(),
        locator: None,
        locator_blocked: Default::default(),
        scene: assets.scene.clone(),
        detail_visibility: Default::default(),
        prepared: Default::default(),
        markers: Default::default(),
        navigator: Default::default(),
        navigator_owners: Default::default(),
        navigator_bounds: Default::default(),
        navigator_pressed: Default::default(),
        tutorial_go_to_block_pressed: Default::default(),
        tooltip: None,
        tooltip_state: None,
        pause_banner: Default::default(),
        icon_tooltip_dwell: Default::default(),
        navigation: Default::default(),
        navigation_bounds: Default::default(),
        navigation_pressed: Default::default(),
        double_click: Default::default(),
        strategic: Default::default(),
        return_camera: None,
        reframe_pending: Default::default(),
        information: None,
        details_visible: Default::default(),
        details_pressed: Default::default(),
        toolbar_input: Default::default(),
        toolbar_right_held: Default::default(),
        toolbar_bounds: Default::default(),
        toolbar_regions: Default::default(),
        detail_icons: Default::default(),
        detail_bounds: Default::default(),
        camera: camera,
        controls: openmt2001::camera::BlockCamera {
            center: [0.; 3],
            angle: 0.,
            height: 0.,
            radius: 0.,
        },
        building_angle: Default::default(),
        building_camera: Default::default(),
        inertia: openmt2001::camera::BlockCameraInertia::new(camera.eye, camera.target),
        keys: Default::default(),
        taps: Default::default(),
        cursor: None,
        mouse_buttons: Default::default(),
        drag: Default::default(),
        pointer: None,
        hovered: None,
        selected: None,
        title_state: None,
        marker_state: None,
        started: std::time::Instant::now(),
        tick: Default::default(),
    }
}
fn snapshot(v: &Inspection, a: &Assets) -> String {
    let g = v.ui_graph.as_ref().unwrap();
    format!(
        "{:?}|{:?}|{:?}",
        g.main,
        g.panel(PanelId::A).unwrap(),
        a.ui_text.fonts()
    )
}
fn business() -> Business {
    let mut pointers = BTreeMap::new();
    for at in [
        0x1c, 0x20, 0x50, 0x54, 0x58, 0x64, 0x88, 0x8c, 0x90, 0x98, 0xac, 0xc8,
    ] {
        pointers.insert(at, 123);
    }
    pointers.insert(0xa4, 456);
    Business {
        pointers,
        nodes: BTreeMap::from([
            (123, Node::Visual(regression_tests::widget())),
            (
                456,
                Node::Text {
                    shown: 1,
                    text: regression_tests::text(),
                },
            ),
        ]),
        selector: None,
        category28: 8,
        selection_mode2c: 3,
        option34: 1,
        action48: 7,
        active4c: 1,
        cacheb4: 1,
        currency: Currency {
            raw_symbol: b"$".to_vec(),
            suffix: 0,
            buffer: Vec::new(),
            prefix_format: b"%s%i".to_vec(),
            suffix_format: b"%i%s".to_vec(),
        },
    }
}
#[test]
#[ignore = "supplied real Intro assets; root owns execution lane"]
fn caller_process_access_step_check_discard_and_commit() {
    let mut a = assets();
    let mut v = inspection(&a);
    let mut o = regression_tests::owner(None);
    let manager = a.ui_text.fonts() as *const _;
    let step = case_zero::child().enter;
    let before = snapshot(&v, &a);
    let raw = o.capture();
    {
        let access = ProcessTextAccess {
            view: &mut v,
            resources: &mut a.ui_text,
        };
        (step.check)(&access, &o, None).unwrap();
    }
    assert_eq!(
        snapshot(&v, &a),
        before,
        "read-only Step check publishes no scratch/selector/Text writes"
    );
    v.ui_graph.as_mut().unwrap().main.notice_text.bounds[1] = f32::NAN;
    let broken = snapshot(&v, &a);
    {
        let mut access = ProcessTextAccess {
            view: &mut v,
            resources: &mut a.ui_text,
        };
        assert!((step.check)(&access, &o, None).is_err());
        assert!((step.apply)(&mut access, &mut o).is_err());
    }
    assert_eq!(snapshot(&v, &a), broken);
    v.ui_graph.as_mut().unwrap().main.notice_text.bounds[1] = 0.;
    (step.apply)(
        &mut ProcessTextAccess {
            view: &mut v,
            resources: &mut a.ui_text,
        },
        &mut o,
    )
    .unwrap();
    assert_ne!(snapshot(&v, &a), before);
    assert_eq!(
        v.ui_graph.as_ref().unwrap().main.notice_caption,
        Some(0x589)
    );
    assert!(v.ui_graph.as_ref().unwrap().main.notice_text.group.count > 0);
    assert_eq!(manager, a.ui_text.fonts() as *const _);
    assert_eq!(o.capture(), raw);
}
#[test]
#[ignore = "supplied real Intro assets; root owns execution lane"]
fn caller_registered_context_navigation_lends_actual_manager() {
    let mut a = assets();
    let mut v = inspection(&a);
    let mut o = regression_tests::owner(None);
    let manager = a.ui_text.fonts() as *const _;
    let before = snapshot(&v, &a);
    let raw = o.capture();
    let mut physical = super::super::PhysicalInput::default();
    let joins = bindings::joins(None);
    {
        let mut context = UiContext::new(
            &mut v,
            &mut physical,
            &mut a,
            &joins,
            SoundBoundary::Existing2d,
        )
        .unwrap();
        //The same production method UiContext::navigate calls. Tests do not
        //claim the still-unjoined C suppliers/full raw navigation boundary.
        context
            .navigation_ui(&mut o, NavigationUi::ReturnMode0ThenToolbar1)
            .unwrap();
    }
    assert_eq!(v.ui_graph.as_ref().unwrap().main.mode, 1);
    assert_ne!(
        snapshot(&v, &a),
        before,
        "actual property enter/notice effects committed"
    );
    assert_eq!(manager, a.ui_text.fonts() as *const _);
    assert_eq!(o.capture(), raw);
    //Intro's live choices remain None. Registered bindings remain optional.
    assert!(joins.tutorial.is_none());
    let before = snapshot(&v, &a);
    let error = joins.primary_clear.unwrap()(&mut v, &mut a, &mut o).unwrap_err();
    assert!(error.contains("unimplemented"), "{error}");
    assert!(!error.contains("resource borrow absent"), "{error}");
    assert_eq!(
        snapshot(&v, &a),
        before,
        "missing real C family still rejects before focus/raw mutation"
    );
    assert_eq!(o.capture(), raw);
}
#[test]
#[ignore = "supplied real Intro assets; root owns execution lane"]
fn caller_group_bridge_main_phase1_phase11_commit_and_error() {
    let mut a = assets();
    let mut v = inspection(&a);
    let mut o = regression_tests::owner(None);
    let manager = a.ui_text.fonts() as *const _;
    let mut providers = MissingFamilies;
    {
        let g = v.ui_graph.as_mut().unwrap();
        g.main.transition_pending = 1;
        g.panel_mut(PanelId::A).unwrap().requested = 0;
        g.panel_mut(PanelId::A).unwrap().frame = 9;
    }
    let before = snapshot(&v, &a);
    ui_graph::check_visibility(
        &ProcessTextAccess {
            view: &mut v,
            resources: &mut a.ui_text,
        },
        PanelId::A,
        true,
        &o,
    )
    .unwrap();
    assert_eq!(snapshot(&v, &a), before);
    v.ui_graph
        .as_mut()
        .unwrap()
        .panel_mut(PanelId::A)
        .unwrap()
        .requested = 1;
    Bridge {
        view: &mut v,
        owner: &mut o,
        assets: &mut a,
        providers: &mut providers,
    }
    .generic14(
        WidgetId {
            panel: PanelId::A,
            kind: WidgetKind::Panel,
        },
        11,
        0,
        0,
    )
    .unwrap();
    assert_eq!(v.ui_graph.as_ref().unwrap().main.transition_pending, 0);
    assert_eq!(
        v.ui_graph.as_ref().unwrap().main.notice_caption,
        Some(0x589)
    );
    v.ui_graph.as_mut().unwrap().main.transition_pending = 1;
    Bridge {
        view: &mut v,
        owner: &mut o,
        assets: &mut a,
        providers: &mut providers,
    }
    .direct18_visible1(PanelId::A)
    .unwrap();
    assert_eq!(v.ui_graph.as_ref().unwrap().main.transition_pending, 0);
    {
        let m = &mut v.ui_graph.as_mut().unwrap().main;
        m.transition_pending = 1;
        m.notice_text.font = 99;
    }
    let broken = snapshot(&v, &a);
    let error = Bridge {
        view: &mut v,
        owner: &mut o,
        assets: &mut a,
        providers: &mut providers,
    }
    .generic14(
        WidgetId {
            panel: PanelId::A,
            kind: WidgetKind::Panel,
        },
        11,
        0,
        0,
    )
    .unwrap_err();
    assert!(error.contains("font99"));
    assert_eq!(snapshot(&v, &a), broken);
    assert_eq!(manager, a.ui_text.fonts() as *const _);
}
#[test]
#[ignore = "supplied real Intro assets; root owns execution lane"]
fn caller_null_reset_business_aliases_discard_then_retry() {
    let mut a = assets();
    let mut v = inspection(&a);
    let mut o = regression_tests::owner(None);
    let manager = a.ui_text.fonts() as *const _;
    let raw = o.capture();
    {
        let m = &mut v.ui_graph.as_mut().unwrap().main;
        m.business_reset = Some(business());
        m.five_reset = Some(Five { state24: 77 });
        let Node::Text { text, .. } = m
            .business_reset
            .as_mut()
            .unwrap()
            .nodes
            .get_mut(&456)
            .unwrap()
        else {
            unreachable!()
        };
        text.font = 99;
    }
    let broken = snapshot(&v, &a);
    let error = main_panel::reset_no_selection(
        &mut ProcessTextAccess {
            view: &mut v,
            resources: &mut a.ui_text,
        },
        &mut o,
    )
    .unwrap_err();
    assert!(error.contains("font99"));
    assert_eq!(snapshot(&v, &a), broken);
    {
        let m = &mut v.ui_graph.as_mut().unwrap().main;
        let Node::Text { text, .. } = m
            .business_reset
            .as_mut()
            .unwrap()
            .nodes
            .get_mut(&456)
            .unwrap()
        else {
            unreachable!()
        };
        text.font = 0;
    }
    main_panel::reset_no_selection(
        &mut ProcessTextAccess {
            view: &mut v,
            resources: &mut a.ui_text,
        },
        &mut o,
    )
    .unwrap();
    let m = &v.ui_graph.as_ref().unwrap().main;
    assert_eq!(m.five_reset.as_ref().unwrap().state24, 0);
    assert_eq!(
        m.property.title.caption,
        super::super::property_child::Caption::Id(30)
    );
    assert!(m.info_text.group.count > 0);
    assert_eq!(m.info_text.colors[0], 16);
    let b = m.business_reset.as_ref().unwrap();
    assert_eq!(b.nodes.len(), 2);
    assert_eq!(b.currency.buffer, b"$0\0");
    for at in [
        0x1c, 0x20, 0x50, 0x54, 0x58, 0x64, 0x88, 0x8c, 0x90, 0x98, 0xac, 0xc8,
    ] {
        assert_eq!(
            b.pointers[&at], 123,
            "actual offset aliases still name one node"
        );
    }
    assert_eq!(manager, a.ui_text.fonts() as *const _);
    assert_eq!(o.capture(), raw);
}
#[test]
#[ignore = "supplied real Intro assets; root owns execution lane"]
fn caller_process_access_keeps_divergent_cache_current_and_active_page() {
    let mut a = assets();
    let mut v = inspection(&a);
    let fonts = a.ui_text.fonts_mut();
    fonts.select(0).unwrap();
    fonts.direct_index(1).unwrap();
    fonts.fonts.get_mut(&0).unwrap().scale = 2.25;
    {
        let mut access = ProcessTextAccess {
            view: &mut v,
            resources: &mut a.ui_text,
        };
        main_panel::set_notice_identifier(&mut access, 0x589).unwrap();
        access.main_mut().unwrap().notice_text.scale = 1.75;
        let packets = main_panel::paint_notice(
            &mut access,
            &case_zero::Input {
                pointer: [0; 2],
                placement: [0.; 2],
                tick: 0,
            },
            &[0xffffffff; 256],
        )
        .unwrap();
        let glyphs = packets
            .iter()
            .filter_map(|p| match p {
                Packet::Glyph(g) => Some(g),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!glyphs.is_empty());
        assert!(glyphs.iter().all(|g| g.font == 1));
    }
    assert_eq!(
        (a.ui_text.fonts().selector.cached, a.ui_text.fonts().current),
        (0, 1)
    );
    assert_eq!(a.ui_text.fonts().fonts[&0].scale, 2.25);
    assert_eq!(a.ui_text.fonts().fonts[&1].scale, 1.);
}
#[test]
#[ignore = "supplied real Intro assets; root owns execution lane"]
fn caller_retains_one_box_across_park_attach_and_rejects_missing_families() {
    let mut a = assets();
    let mut first = inspection(&a);
    let mut o = regression_tests::owner(None);
    let manager = a.ui_text.fonts() as *const _;
    let mut parked = Some(Parked::take_from(&mut first).unwrap());
    assert!(first.ui_graph.is_none());
    drop(first);
    let mut second = inspection(&a);
    second.ui_graph = None;
    second.camera_navigation = None;
    Parked::put_into(&mut parked, &mut second).unwrap();
    assert!(parked.is_none());
    let before = snapshot(&second, &a);
    let mut access = ProcessTextAccess {
        view: &mut second,
        resources: &mut a.ui_text,
    };
    assert!(
        main_panel::switch(&mut access, 2, &mut o)
            .unwrap_err()
            .contains("unimplemented")
    );
    assert!(
        main_panel::switch(&mut access, 5, &mut o)
            .unwrap_err()
            .contains("unimplemented")
    );
    assert!(
        ui_graph::set_visibility(&mut access, PanelId::B, true, &mut o)
            .unwrap_err()
            .contains("unimplemented")
    );
    drop(access);
    assert_eq!(snapshot(&second, &a), before);
    assert_eq!(manager, a.ui_text.fonts() as *const _);
    drop(second);
    let moved = a;
    assert_eq!(manager, moved.ui_text.fonts() as *const _);
}
