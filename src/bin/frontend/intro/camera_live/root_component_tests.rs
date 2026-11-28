use super::*;
use openmt2001::camera::primary::CursorRequest;
#[test]
fn cursor_equal_priority_override_and_real_presentation_decay() {
    let mut cursor = Cursor::new(2, 1).unwrap();
    cursor
        .select(
            CursorRequest {
                state: 1,
                priority: 0,
            },
            false,
        )
        .unwrap();
    assert_eq!(cursor.host_index(), 1);
    cursor
        .select(
            CursorRequest {
                state: 3,
                priority: 1,
            },
            false,
        )
        .unwrap();
    assert_eq!(cursor.host_index(), 2);
    cursor.presented();
    cursor
        .select(
            CursorRequest {
                state: 1,
                priority: 0,
            },
            false,
        )
        .unwrap();
    assert_eq!(cursor.host_index(), 3);
    cursor
        .select(
            CursorRequest {
                state: 2,
                priority: 0,
            },
            true,
        )
        .unwrap();
    assert_eq!(cursor.host_index(), 0);
    assert!(
        Cursor::new(8, 0).is_err(),
        "states beyond the eight retained images remain unsupported"
    );
}
#[test]
fn physical_edges_and_delta_survive_routing_and_consume_once() {
    use winit::{
        event::MouseButton,
        keyboard::{KeyCode, PhysicalKey},
    };
    let mut physical = PhysicalInput::default();
    physical.key(PhysicalKey::Code(KeyCode::ArrowRight), true);
    physical.key(PhysicalKey::Code(KeyCode::ArrowRight), false);
    physical.button(MouseButton::Right, true);
    physical.motion([3., -5.]);
    assert_eq!(
        physical.snapshot().unwrap(),
        ([true, false, false, false], [false, true], [3, -5])
    );
    physical.consumed();
    assert_eq!(
        physical.snapshot().unwrap(),
        ([false; 4], [false, true], [0; 2])
    );
    physical.key(PhysicalKey::Code(KeyCode::ArrowLeft), true);
    physical.unfocus();
    assert_eq!(
        physical.snapshot().unwrap(),
        ([false; 4], [false; 2], [0; 2])
    );
}
#[test]
fn original_graphics_policy_is_explicit_and_not_current_gpu_backend() {
    let d3d = Globals::after_normal_initialization(OriginalGraphicsPolicy::NormalD3d, false);
    let software =
        Globals::after_normal_initialization(OriginalGraphicsPolicy::SoftwareKind2Or3, false);
    assert!(!d3d.legacy_yaw && d3d.low_height);
    assert!(!software.legacy_yaw && !software.low_height);
}
