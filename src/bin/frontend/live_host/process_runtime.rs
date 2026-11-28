//! The process runtime moved into the library (`openmt2001::process_runtime`,
//! #99). This keeps `crate::live_host::process_runtime::*` paths working.
pub use openmt2001::process_runtime::*;

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::process_runtime::test_fixture as fixture;

    #[test]
    fn host_birth_does_not_reset_the_count() {
        let p = fixture();
        p.increment_count4ba580().unwrap();
        let mut host = crate::live_host::Host::new([0.; 3]);
        host.process_runtime = Some(p.clone());
        assert_eq!(p.count822d38().unwrap(), 1, "Host birth is no reset");
    }
    #[test]
    fn actual_lua_command_consumer_writes_source_before_projection() {
        use crate::live_host::apply_source_picking_command as apply;
        use openmt2001::{level_script::Command as C, lua4::Value};
        let p = fixture();
        apply(&p, &C::TutorialDisableAllPicking, &[]).unwrap();
        assert_eq!(p.byte(Byte::Controller920888).unwrap(), 0);
        apply(&p, &C::TutorialEnableBlockPicking, &[Value::Number(3.9)]).unwrap();
        assert_eq!(p.picking_projection().unwrap().block_id, 3);
        apply(
            &p,
            &C::TutorialEnableBuildingPicking,
            &[Value::Bytes(b"not numeric".to_vec())],
        )
        .unwrap();
        let x = p.picking_projection().unwrap();
        assert_eq!(
            (x.common, x.block, x.building, x.building_id),
            (1, 0, 1, -1)
        );
        apply(&p, &C::TutorialDisableAllPicking, &[]).unwrap();
        apply(&p, &C::TutorialEnableAllPicking, &[]).unwrap();
        assert_eq!(p.byte(Byte::Controller920888).unwrap(), 1);
    }
    #[test]
    fn actual_picking_conversion_keeps_native_low_word_and_indefinite_semantics() {
        use crate::live_host::{picking_ftol, tutorial_pick_target};
        use openmt2001::lua4::Value;
        for (value, expected) in [
            (3.9, 3),
            (-2.9, -2),
            (4294967305.75, 9),
            (-1.9, -1),
            (f64::NAN, 0),
            (f64::INFINITY, 0),
            (f64::NEG_INFINITY, 0),
            (9223372036854775808., 0),
        ] {
            assert_eq!(picking_ftol(value), expected);
        }
        assert_eq!(tutorial_pick_target(&[Value::Number(-1.9)]).unwrap(), None);
        assert_eq!(
            tutorial_pick_target(&[Value::Number(-2.9)]).unwrap(),
            Some((-2i32) as u32)
        );
    }
}
