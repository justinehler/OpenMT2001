//! Finite regressions. Explicit synthetic rectangles/records
//! exercise generic mechanics; none claims actual F nonzero title/caption.
use super::*;
use crate::navigator::upper::Inputs;
use openmt2001::native_input::{LogicalPointer, MousePacket};
use std::sync::Arc;
fn fixture() -> Plan {
    let mut panel = Panel::bound(Group::Primary, 10, 9, [0., 0.], false, None, 255).unwrap();
    panel.requested = 1;
    panel.f_pointer = Some(PanelCells {
        drag346: 0,
        pending354: 0,
        offsets: [7., 9.],
        velocity_z: 13.,
        title164: [0.; 4],
        caption18c: [0.; 4],
        caption_region340: u32::MAX,
        descriptor: Some(0x600000),
        reference: Some(0x610000),
    });
    let geometry = Geometry {
        bounds: [0., 0., 100., 100.],
        parent_open: [0., 0., 100., 100.],
        parent_closed: [0.; 4],
        second: [0.; 4],
        backing: [0.; 4],
    };
    let upper = State::installed(
        geometry,
        -1,
        0,
        0,
        Pointer {
            xy: [-123, -456],
            blocked: 7,
            restricted: 0,
            allowed: -1,
        },
        Arc::new(Inputs {
            source_epoch: crate::live_host::camera_bridge::PropertyEpoch::synthetic(1),
            rows: [[0; 380]; 52],
            owners: [None; 52],
            count: 0,
            phase: 0,
            presentation_mode: 0,
            property_labels: [0; 52],
        }),
    );
    let input = RetainedCells {
        mouse_current: MousePacket([0; 16]),
        mouse_prior: MousePacket([0; 16]),
        keyboard_current: [0; 256],
        keyboard_prior: [0; 256],
        logical_pointer: LogicalPointer { x: 50, y: 50 },
        left_override: 0,
        right_override: 0,
        double_click: 0,
        keyboard_poll_tick: 0,
        device_event_count: 16,
        input_enabled: 1,
        focus_transition: 0,
        recording: 0,
        space_recorded: 0,
    };
    Plan {
        panel,
        upper,
        input,
        models: Models {
            manager: 0x700000,
            references: BTreeMap::from([(0x610000, 1)]),
            slots: BTreeMap::from([(
                1,
                ModelSlot {
                    available: 1,
                    model: Some((0x710000, 10)),
                },
            )]),
        },
        selected: Some(F),
        frame_parents: vec![(F, None)],
        override590504: None,
        modal: 0,
        blocked: 0,
        cursor: Cursor::new(2, 0).unwrap(),
        force_default: false,
        observed: Vec::new(),
    }
}
fn buttons(p: &mut Plan, left: u8, prior: u8, right: u8) {
    p.input.mouse_current.0[12] = left;
    p.input.mouse_current.0[13] = right;
    p.input.mouse_prior.0[12] = prior;
}
fn category(p: &mut Plan, x: f32, z: f32) {
    let input = Arc::make_mut(&mut p.upper.input);
    input.count = 1;
    for (offset, value) in [(0xd0, 100f32), (0xd4, 100.), (0xe0, x), (0xe4, z)] {
        input.rows[0][offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
fn seen(p: &Plan, name: &str) -> usize {
    p.observed.iter().position(|s| *s == name).unwrap()
}
#[test]
fn strict_x_inclusive_y_and_any_nonzero_right() {
    let mut p = fixture();
    let r = [0., 0., 100., 100.];
    for (x, y, expected) in [
        (0, 50, false),
        (100, 50, false),
        (50, 0, true),
        (50, 100, true),
        (50, -1, false),
    ] {
        p.input.logical_pointer = LogicalPointer { x, y };
        assert_eq!(p.hit(r).unwrap(), expected);
    }
    p.input.logical_pointer = LogicalPointer { x: 50, y: 50 };
    p.input.mouse_current.0[13] = 2;
    assert!(!p.hit(r).unwrap());
    p.cells_mut().unwrap().descriptor = None;
    p.input.mouse_current.0[13] = 0;
    assert!(!p.hit(r).unwrap());
    assert_eq!(slot20(), 1);
    p.cells_mut().unwrap().descriptor = Some(0x600000);
    p.panel.position = [1., 0.];
    p.input.logical_pointer = LogicalPointer {
        x: 16_777_217,
        y: 50,
    };
    assert!(
        !p.hit([16_777_216., 0., 16_777_220., 100.]).unwrap(),
        "PC64 left sum equals pointer; premature F32 sum would admit"
    );
    p.input.logical_pointer.x = 16_777_218;
    assert!(p.hit([16_777_216., 0., 16_777_220., 100.]).unwrap());
}
#[test]
fn containment_uses_parent_open_while_moving_not_map() {
    let mut p = fixture();
    p.panel.requested = 0;
    p.panel.moving = 1;
    p.upper.geometry.bounds = [200., 200., 300., 300.];
    assert!(p.base_hit().unwrap());
    p.panel.moving = 0;
    assert!(!p.base_hit().unwrap());
    //+2c has no draw/disable/global gate. Generic14 does.
    p.panel.requested = 1;
    p.panel.draw_enabled = 0;
    p.panel.disabled1d = 1;
    p.blocked = 1;
    assert!(p.base_hit().unwrap());
}
#[test]
fn busy_blocks_pointer_suffix_but_generic11_updates_hover_and_cursor() {
    for event in [4, 5, 6] {
        let mut p = fixture();
        category(&mut p, 0., 0.);
        buttons(&mut p, 1, 1, 0);
        p.panel.moving = 1;
        p.generic(event, 0).unwrap();
        assert_eq!(p.upper.selected, -1);
        assert!(p.observed.is_empty());
    }
    let mut p = fixture();
    category(&mut p, 0., 0.);
    p.panel.moving = 1;
    p.generic(11, 0).unwrap();
    assert_eq!(p.upper.selected, 0);
    assert_eq!(p.cursor.host_index(), 3);
    assert!(seen(&p, "43dd90 hover") < seen(&p, "43d130 suffix"));
}
#[test]
fn drag_continues_before_busy_and_release346_one_differs_from_two() {
    for drag in [1, 2] {
        let mut p = fixture();
        p.cells_mut().unwrap().drag346 = drag;
        p.cells_mut().unwrap().pending354 = 73;
        p.cells_mut().unwrap().offsets = [2., 3.];
        p.override590504 = Some(F);
        p.panel.moving = 1;
        buttons(&mut p, 0, 1, 0);
        p.generic(6, 0).unwrap();
        assert_eq!(p.panel.position, [48., 47.]);
        assert_eq!(p.cells().unwrap().pending354, 0);
        assert_eq!(p.cells().unwrap().drag346, if drag == 1 { 0 } else { 2 });
        assert_eq!(p.override590504, if drag == 1 { None } else { Some(F) });
        assert!(seen(&p, "descriptor Y store") < seen(&p, "pending clear"));
        assert_eq!(p.upper.enabled, 0);
    }
}
#[test]
fn current_buttons_drive_cleanup_even_when_event_label_is_press() {
    let mut p = fixture();
    p.cells_mut().unwrap().drag346 = 1;
    p.override590504 = Some(F);
    p.cells_mut().unwrap().pending354 = 9;
    buttons(&mut p, 1, 0, 2);
    p.generic(4, 0).unwrap();
    assert_eq!(p.cells().unwrap().drag346, 0);
    assert_eq!(p.cells().unwrap().pending354, 0);
    assert_eq!(p.override590504, None);
    assert_eq!(
        p.upper.enabled, 1,
        "suffix follows original event after cleanup"
    );
}
#[test]
fn pending_guard_skips_both_toggle_and_drag_start() {
    let mut p = fixture();
    p.cells_mut().unwrap().pending354 = 7;
    p.cells_mut().unwrap().title164 = [0., 0., 100., 100.];
    p.cells_mut().unwrap().caption18c = [0., 0., 100., 100.];
    buttons(&mut p, 1, 0, 0);
    p.generic(4, 0).unwrap();
    assert_eq!(p.panel.requested, 1);
    assert_eq!(p.cells().unwrap().drag346, 0);
    assert_eq!(p.upper.enabled, 1);
    assert!(!p.observed.contains(&"F43ab00 RET"));
}
#[test]
fn names_null_zero_title_caption_are_negative_hits_without_removing_generic_branch() {
    let mut p = fixture();
    buttons(&mut p, 1, 0, 0);
    p.generic(4, 0).unwrap();
    assert_eq!(p.panel.requested, 1);
    assert_eq!(p.cells().unwrap().drag346, 0);
    assert_eq!(p.upper.enabled, 1);
    let mut closed = fixture();
    closed.panel.requested = 0;
    buttons(&mut closed, 1, 0, 0);
    closed.generic(4, 0).unwrap();
    assert_eq!(closed.panel.requested, 0);
    assert!(closed.observed.is_empty());
}
#[test]
fn close_ret_precedes_selected_frame_parent_clear_and_reset_is_zyx() {
    let mut p = fixture();
    p.panel.independent = 1;
    p.panel.position = [6., 9.];
    p.panel.velocity = [3., 4.];
    p.cells_mut().unwrap().title164 = [0., 0., 100., 100.];
    let child = WidgetId {
        panel: PanelId::E,
        kind: WidgetKind::Unported(4),
    };
    p.selected = Some(child);
    p.frame_parents.push((child, Some(PanelId::F)));
    p.input.keyboard_current[0x1d] = 2;
    buttons(&mut p, 1, 0, 0);
    p.generic(4, 0).unwrap();
    assert_eq!(
        (p.panel.requested, p.panel.moving, p.selected),
        (0, 0, None)
    );
    assert_eq!(p.panel.velocity, [0.; 2]);
    assert_eq!(p.cells().unwrap().velocity_z, 0.);
    assert!(seen(&p, "F43ab00 RET") < seen(&p, "selected clear"));
    assert!(seen(&p, "velocity Z clear") < seen(&p, "velocity Y clear"));
    assert!(seen(&p, "velocity Y clear") < seen(&p, "velocity X clear"));
    assert!(!p.observed.contains(&"recursive slot14(11)"));
    let mut prior = fixture();
    prior.panel.independent = 1;
    prior.panel.transition_end = 1;
    prior.panel.velocity = [3., 4.];
    prior.cells_mut().unwrap().title164 = [0., 0., 100., 100.];
    prior.input.keyboard_prior[0x1d] = 2;
    buttons(&mut prior, 1, 0, 0);
    prior.generic(4, 0).unwrap();
    assert_eq!(prior.panel.velocity, [3., 4.]);
    assert_eq!(
        prior.cells().unwrap().velocity_z,
        13.,
        "prior keyboard cell does not reset steps"
    );
}
#[test]
fn open_ret_precedes_recursive11_and_model_backed_outer_event_stays_busy() {
    let mut p = fixture();
    p.panel.requested = 0;
    p.upper.geometry.parent_closed = [0., 0., 100., 100.];
    category(&mut p, 0., 0.);
    buttons(&mut p, 1, 0, 0);
    p.generic(4, 0).unwrap();
    assert_eq!((p.panel.requested, p.panel.moving), (1, 1));
    assert_eq!(p.upper.selected, 0);
    assert_eq!(p.upper.enabled, 0, "recursive11 does not invent4/5");
    assert!(seen(&p, "F43aaf0 RET") < seen(&p, "recursive slot14(11)"));
    assert!(seen(&p, "recursive slot14(11)") < seen(&p, "43dd90 hover"));
    assert_eq!(p.cursor.host_index(), 3);
}
#[test]
fn independent_open_recursive11_then_outer_press_and_shared_override_drag() {
    let mut p = fixture();
    p.panel.requested = 0;
    p.panel.independent = 1;
    p.upper.geometry.parent_closed = [0., 0., 100., 100.];
    p.cells_mut().unwrap().caption18c = [0., 0., 100., 100.];
    p.models.manager = 0;
    buttons(&mut p, 1, 0, 0);
    p.generic(4, 0).unwrap();
    assert_eq!(
        (p.panel.requested, p.panel.moving, p.upper.enabled),
        (1, 0, 1)
    );
    assert_eq!(p.cells().unwrap().drag346, 1);
    assert_eq!(p.cells().unwrap().offsets, [50., 50.]);
    assert_eq!(p.override590504, Some(F));
    assert!(seen(&p, "recursive slot14(11)") < seen(&p, "drag offsets/override publish"));
}
#[test]
fn another_shared_override_blocks_drag_and_own_override_allows_it() {
    let other = WidgetId {
        panel: PanelId::A,
        kind: WidgetKind::Panel,
    };
    for owner in [Some(other), Some(F), None] {
        let mut p = fixture();
        p.override590504 = owner;
        p.cells_mut().unwrap().caption18c = [0., 0., 100., 100.];
        buttons(&mut p, 1, 1, 0);
        p.generic(4, 0).unwrap();
        assert_eq!(
            p.cells().unwrap().drag346,
            if owner == Some(other) { 0 } else { 1 }
        );
        assert_eq!(
            p.override590504,
            if owner == Some(other) { owner } else { Some(F) }
        );
    }
}
#[test]
fn live_model_metadata_missing_or_replaced_never_falls_back_to_frame_count() {
    let mut p = fixture();
    p.panel.requested = 0;
    p.upper.geometry.parent_closed = [0., 0., 100., 100.];
    buttons(&mut p, 1, 0, 0);
    p.panel.frame_count = 999;
    p.models.slots.get_mut(&1).unwrap().available = 0;
    assert!(p.generic(4, 0).unwrap_err().contains("no fallback"));
    assert_eq!(p.panel.requested, 0);
    p.models.slots.get_mut(&1).unwrap().available = 1;
    p.models.slots.get_mut(&1).unwrap().model = None;
    assert!(p.generic(4, 0).unwrap_err().contains("metadata"));
    assert_eq!(p.panel.requested, 0);
    p.cells_mut().unwrap().reference = Some(0x620000);
    assert!(p.generic(4, 0).unwrap_err().contains("first-handle"));
    p.models.references.insert(0x620000, 2);
    p.models.slots.insert(
        2,
        ModelSlot {
            available: 2,
            model: Some((0x720000, 0)),
        },
    );
    //Actual U16 zero still yields code1. Cached frame_count999 is unrelated.
    assert_eq!(p.models.frames(p.cells().unwrap()).unwrap(), 0);
    p.generic(4, 0).unwrap();
    assert_eq!(p.panel.requested, 1);
}
#[test]
fn generic_hidden_disabled_and_leave_have_distinct_suffix_rules() {
    let mut p = fixture();
    p.upper.enabled = 1;
    p.generic(3, 0).unwrap();
    assert_eq!(p.upper.enabled, 1);
    p.specialized(3, 0).unwrap();
    assert_eq!(p.upper.enabled, 0);
    p.panel.requested = 0;
    p.upper.selected = 77;
    p.generic(11, 0).unwrap();
    assert_eq!(p.upper.selected, 77);
    p.panel.requested = 1;
    p.panel.disabled1d = 1;
    p.generic(4, 0).unwrap();
    assert_eq!(p.upper.enabled, 0);
    p.generic(11, 0).unwrap();
    assert_eq!(
        p.upper.selected, 77,
        "disabled1d suppresses hover even when11 admitted"
    );
}
#[test]
fn modal_exact_one_differs_from_two_and_positive5_discards_prospective_stores() {
    let mut live = fixture();
    category(&mut live, -100., 100.);
    live.upper.enabled = 1;
    live.upper.selected = 9;
    live.cells_mut().unwrap().drag346 = 2;
    live.cells_mut().unwrap().offsets = [40., 40.];
    buttons(&mut live, 1, 1, 0);
    let before = (
        live.panel.position,
        live.upper.selected,
        live.upper.enabled,
        live.cursor.host_index(),
        live.override590504,
    );
    live.modal = 2;
    assert!(
        evaluate(live.clone(), 5, 0, false).is_err(),
        "production evaluation returns no publishable plan"
    );
    let mut prospective = live.clone();
    prospective.modal = 2;
    assert!(
        prospective
            .generic(5, 0)
            .unwrap_err()
            .contains("positive specialized5")
    );
    assert_eq!(
        prospective.panel.position,
        [10., 10.],
        "proves rejection after real prospective drag operations"
    );
    assert_eq!(
        (
            live.panel.position,
            live.upper.selected,
            live.upper.enabled,
            live.cursor.host_index(),
            live.override590504
        ),
        before
    );
    let mut modal = live.clone();
    modal.modal = 1;
    modal.generic(5, 0).unwrap();
    assert_eq!(modal.upper.selected, 9);
    //A direct1 can run while moving; generic hidden11 cannot reach it.
    modal.panel.moving = 1;
    modal.specialized(1, 0).unwrap();
}

fn empty_cells() -> Cells {
    //Minimal synthetic PE32 header with no sections. This is a parser fixture,
    //not original bytes or a claim that native heap allocations are zero.
    let mut pe = vec![0; 0x98];
    pe[..2].copy_from_slice(b"MZ");
    pe[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    pe[0x40..0x44].copy_from_slice(b"PE\0\0");
    pe[0x44..0x46].copy_from_slice(&0x14cu16.to_le_bytes());
    pe[0x54..0x56].copy_from_slice(&64u16.to_le_bytes());
    pe[0x58..0x5a].copy_from_slice(&0x10bu16.to_le_bytes());
    pe[0x74..0x78].copy_from_slice(&0x400000u32.to_le_bytes());
    Cells::process_allocation(&pe).unwrap()
}
#[test]
fn factory_retains_ctor_zero_regions_and_poisoned_unwritten_offsets() {
    let mut cells = empty_cells();
    let f = 0x58cdb0;
    cells.heap_allocation(f, 0x2000).unwrap();
    cells.write(f, &vec![0xa5; 0x2000]).unwrap();
    super::super::native_initializers::panel43b380_scalars(&mut cells, f).unwrap();
    cells.set_word(f, 0x525578).unwrap();
    //Supplied embedded TextControl factory product: node left/top zero and
    //440540 terminal right/bottom zero. It is distinct from focus caption.
    for offset in [0x18c, 0x190, 0x194, 0x198] {
        cells.set_word(f + offset, 0).unwrap();
    }
    //Actual names-null43b770 result;43aca0(-1) preserves each bound preimage.
    cells.set_word(f + 0x33c, u32::MAX).unwrap();
    cells.set_word(f + 0x340, u32::MAX).unwrap();
    cells.set_word(f + 0x37c, 0x600000).unwrap();
    cells.heap_allocation(0x600000, 4).unwrap();
    cells.set_word(0x600000, 0).unwrap();
    let captured = PanelCells::capture(&cells, f).unwrap();
    assert_eq!(captured.title164, [0.; 4]);
    assert_eq!(captured.caption18c, [0.; 4]);
    assert_eq!(
        (captured.drag346, captured.pending354, captured.velocity_z),
        (0, 0, 0.)
    );
    assert_eq!(captured.offsets.map(f32::to_bits), [0xa5a5a5a5; 2]);
    assert_eq!(captured.caption_region340, u32::MAX);
    cells.set_word(f + 0x164, 7f32.to_bits()).unwrap();
    cells.set_word(f + 0x18c, 9f32.to_bits()).unwrap();
    let changed = PanelCells::capture(&cells, f).unwrap();
    assert_eq!(changed.title164[0], 7.);
    assert_eq!(changed.caption18c[0], 9.);
    assert!(
        PanelCells::capture(&cells, f + 4).is_err(),
        "no universal subtype/default initializer"
    );
}
