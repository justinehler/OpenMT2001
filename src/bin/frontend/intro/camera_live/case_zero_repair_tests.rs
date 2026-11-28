//! Synthetic regressions without original resources or live UI.
use super::super::{
    case_zero,
    case_zero_text::Literals,
    main_model, navigation,
    property_child::{self, Caption, ListRange, Selector, Title, Widget, Widgets},
    property_inputs, ui_graph, ui_selection,
};
use super::*;
use std::collections::BTreeMap;
pub(crate) fn widget() -> Widget {
    Widget {
        shown: 7,
        native_mode: 1,
        selected: 1,
        disabled: 1,
        down: 1,
        node_disabled: 1,
        bounds: [0., 0., 20., 20.],
        background: [0.; 5],
        image: 19,
        animation: 23,
        completion: 1,
        caption: Caption::Id(9),
        font: 0,
    }
}
pub(crate) use crate::ui_text::case_zero_text::fixtures::{fonts, text};
pub(crate) fn literals() -> Literals {
    Literals {
        version: b"%i.%i".to_vec(),
        space: b" ".to_vec(),
        colors: [1, 2, 3, 4],
    }
}
pub(crate) fn list(key: u32) -> case_zero::List {
    case_zero::List {
        key,
        bounds: [0., 0., 200., 100.],
        shown: 1,
        disabled: 0,
        flag67c: 0,
        primary: text(),
        secondary: text(),
        scratch: text(),
        flags: Vec::new(),
        slots: Vec::new(),
        markers: Vec::new(),
        states: Vec::new(),
        entries: Vec::new(),
        chosen: -1,
        value: 0,
    }
}
pub(crate) fn selector() -> Selector {
    Selector {
        shown: 1,
        admitted191: 1,
        node_disabled: 0,
        extent: 0.,
        simple: None,
        first_list: Some(ListRange {
            limit480: 0,
            limit2cc: 0,
            first284: 10,
        }),
        text_list: None,
        last_list: None,
        previous: widget(),
        next: widget(),
        zero: 0.,
    }
}
pub(crate) fn category() -> property_inputs::Category {
    property_inputs::Category {
        label: 50,
        metadata_group: 1,
        title: 100,
        color: 0xff112233,
        available: 0,
        rights: 0,
        offer_pending: 0,
        rights_pending: 0,
        owner_reference: 0,
        owner_slot: 0,
        owner_name: Vec::new(),
        value: openmt2001::x87::Sum::from_i32(1),
        selected_amount: 1.,
        summary: Vec::new(),
        land: Vec::new(),
    }
}
pub(crate) fn selected() -> Selected {
    Selected {
        key: ui_selection::SelectionKey::NativeObject(0x600000),
        kind: 4,
        category: 0,
        hit: None,
    }
}
pub(crate) fn owner(current: Option<Selected>) -> Owner {
    use openmt2001::camera::bank::{Bank, Capture};
    let mut owner = Owner::new(Bank::from_capture(Capture::from_records([[0; 196]; 2])), 0);
    owner
        .install_ui_selection(ui_selection::SelectionState::installed(
            current, current, None, None, None, None, -1, -1,
        ))
        .unwrap();
    owner
}
pub(crate) fn fixture() -> Prospective {
    let widgets = Widgets {
        previous: Some(widget()),
        next: Some(widget()),
        report_one: Some(widget()),
        selector_one: Some(selector()),
        report_three: None,
        selector_three: None,
        report_four: None,
        selector_four: None,
        report_five: None,
        selector_five: None,
        offer: Some(widget()),
        rights: Some(widget()),
        auxiliary: Some(widget()),
        marker: None,
        text: None,
    };
    let property = property_child::State {
        reset_flag: 1,
        other_group: 0,
        utility_group: 0,
        utility_index: u32::MAX,
        other_index: u32::MAX,
        active: 1,
        mode: 0,
        report: 0,
        profit_period: 4,
        efficiency_period: 3,
        infos: std::array::from_fn(|_| widget()),
        title: Title {
            shown: 7,
            caption: Caption::Id(9),
        },
        notice: Some(widget()),
        widgets,
        rights_explanation: widget(),
        offer_labels: std::array::from_fn(|_| widget()),
        amount: 1.,
        amount_dirty: 0,
        step: 1.,
        minimum: 0.,
        maximum: 10.,
        action_key: 9,
        summary: Vec::new(),
        summary_text: Vec::new(),
        land: Vec::new(),
        land_links: Vec::new(),
        selected_category: u32::MAX,
        colors: [9; 4],
        inputs: property_inputs::Inputs {
            categories: vec![category()],
            utility_labels: [100; 4],
            other_labels: [101; 4],
            current_reference: 7,
            current_slot: 0,
            mode_titles: [21, 22, 23],
            mode_images: [31, 32, 33],
            info_titles: [41; 5],
            info_images: [51; 5],
            special_image: 61,
            inset: 0.1,
            currency_symbol: b"$".to_vec(),
            currency_suffix: false,
        },
    };
    let model = main_model::State {
        geometry: main_model::Geometry {
            frames: vec![
                main_model::Frame {
                    parent: [0., 0., 200., 100.],
                    main: None,
                    caption: None,
                    toggle: [0.; 5]
                };
                10
            ],
        },
        closed_bounds: [0.; 4],
        open_bounds: [0.; 4],
        caption_bounds: [0.; 4],
        caption_visual_bounds: [0.; 4],
        overlay_extent: [0.; 5],
        overlay_image: 0,
        overlay_completion: 1,
        overlay_animation: 0,
        open_icon: 7,
        closed_icon: 8,
    };
    let zero = case_zero::State {
        state: 0,
        list: Some(list(0x610000)),
        indicator: Some(selector()),
        literals: literals(),
    };
    let main = State::installed(
        0,
        1,
        0,
        std::array::from_fn(|_| widget()),
        b"retained".to_vec(),
        Some(9),
        property,
        model,
        text(),
        zero,
        text(),
        None,
        None,
        [None; 6],
    );
    let panel = ui_graph::Panel::bound(
        navigation::Group::Primary,
        10,
        9,
        [0., 0.],
        false,
        None,
        255,
    )
    .unwrap();
    Prospective {
        fonts: fonts(),
        main,
        panel: Some(panel),
        current: None,
    }
}
pub(crate) fn unchanged_on_error(
    p: &mut Prospective,
    operation: impl FnOnce(&mut Prospective) -> Result<(), String>,
) -> String {
    let before = format!("{p:?}");
    let current = p.current;
    let error = transact(p, current, operation).unwrap_err();
    assert_eq!(format!("{p:?}"), before);
    error
}
#[test]
fn parent_switch_discards_notice_exit_and_control_writes_on_title_failure() {
    let mut p = fixture();
    p.main.info_text.font = 99;
    let owner = owner(None);
    let raw = owner.capture();
    let selection = format!("{:?}", owner.ui_selection().unwrap());
    assert!(unchanged_on_error(&mut p, |p| switch_owned(p, 1, &owner)).contains("font99"));
    assert_eq!(owner.capture(), raw);
    assert_eq!(format!("{:?}", owner.ui_selection().unwrap()), selection);
    p.main.info_text.font = 0;
    transact(&mut p, None, |p| switch_owned(p, 1, &owner)).unwrap();
    assert_eq!(p.main.mode, 1);
    assert_eq!(p.main.property.title.caption, Caption::Id(28));
    assert_eq!(p.main.info_text.group.row(0).unwrap(), &[1]);
    assert_eq!(p.main.info_text.colors[0], 16);
}
#[test]
fn transition_entry_discards_pending_and_button_writes_on_bad_identifier() {
    let mut p = fixture();
    p.main.mode = 1;
    p.main.transition_pending = 1;
    p.fonts.language.test_replace(28, vec![5]);
    let owner = owner(None);
    assert!(unchanged_on_error(&mut p, |p| message_owned(p, 11, &owner, 9)).contains("formatting"));
    p.fonts.language.test_replace(28, vec![1]);
    transact(&mut p, None, |p| message_owned(p, 11, &owner, 9)).unwrap();
    assert_eq!(p.main.transition_pending, 0);
}
#[test]
fn null_reset_discards_earlier_model_property_and_list_writes_on_business_title_failure() {
    use super::super::other_child_resets::{Business, Currency, Node};
    let mut p = fixture();
    let owner = owner(None);
    let mut pointers = BTreeMap::new();
    for at in [
        0x1c, 0x20, 0x50, 0x54, 0x58, 0x64, 0x88, 0x8c, 0x90, 0x98, 0xa4, 0xac, 0xc8,
    ] {
        pointers.insert(at, if at == 0xa4 { 0 } else { 123 });
    }
    p.main.business_reset = Some(Business {
        pointers,
        nodes: BTreeMap::from([(123, Node::Visual(widget()))]),
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
    });
    p.fonts.language.test_replace(30, vec![5]);
    assert!(
        unchanged_on_error(&mut p, |p| reset_no_selection_owned(p, &owner)).contains("formatting")
    );
    p.fonts.language.test_replace(30, vec![3]);
    transact(&mut p, None, |p| reset_no_selection_owned(p, &owner)).unwrap();
    assert_eq!(p.main.info_text.group.row(0).unwrap(), &[3]);
    assert_eq!(p.main.info_text.colors[0], 16);
    assert_eq!(p.main.property.title.caption, Caption::Id(30));
    assert_eq!(p.main.case_zero.state, 0);
    assert_eq!(
        p.main.business_reset.as_ref().unwrap().nodes.len(),
        1,
        "aliased offsets retain one live node"
    );
}
#[test]
fn unsupported_parent_input_and_lifecycle_do_not_commit() {
    let mut p = fixture();
    let owner = owner(None);
    assert!(
        unchanged_on_error(&mut p, |p| message_owned(p, 12, &owner, 9)).contains("input/selection")
    );
    assert!(unchanged_on_error(&mut p, |p| switch_owned(p, 2, &owner)).contains("unimplemented"));
    assert!(unchanged_on_error(&mut p, |p| switch_owned(p, 5, &owner)).contains("unimplemented"));
}
