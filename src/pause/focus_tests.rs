//! Literal branch expectations from004bab90/004bac00 and004bacc0.
//! No executable input or production predictor is used as the oracle.
use super::{State, Timers};

#[test]
fn native_focus_signed_counter_word_and_latch_boundaries() {
    // gain, count, latch, ordinary word, blocked, expected count/latch/callback
    let cases = [
        (false, 0, 0, [0; 4], false, 1, 1, Some(Timers::Suspend)),
        (false, 1, 1, [0; 4], false, 1, 1, None),
        (false, 0x7f, 0, [0; 4], false, 0x7f, 0, None),
        (false, 0x80, 0, [0; 4], false, 0x81, 0, None),
        (false, 0xff, 0, [0; 4], false, 0, 0, None),
        (false, 0, 1, [1, 0, 0, 0], false, 1, 1, None),
        (false, 0, 0, [0; 4], true, 0, 0, None),
        (false, 0xff, 1, [0; 4], true, 0xff, 1, None),
        (true, 1, 1, [0; 4], false, 0, 0, Some(Timers::Resume)),
        (true, 2, 1, [0; 4], false, 1, 0, Some(Timers::Resume)),
        (true, 0x7f, 1, [0; 4], false, 0x7e, 0, Some(Timers::Resume)),
        (true, 0, 1, [0; 4], false, 0, 1, None),
        (true, 0x80, 1, [0; 4], false, 0x80, 1, None),
        (true, 0xff, 1, [0; 4], false, 0xff, 1, None),
        (true, 1, 0, [0; 4], false, 0, 0, None),
        (true, 1, 1, [1, 0, 0, 0], false, 0, 1, None),
        (true, 1, 1, [0, 1, 0, 0], false, 0, 1, None),
        (true, 1, 1, [0, 0, 1, 0], false, 0, 1, None),
        (true, 1, 1, [0, 0, 0, 0x80], false, 0, 1, None),
        (true, 1, 1, [0; 4], true, 1, 1, None),
    ];
    for (index, (gain, count, latch, reasons, blocked, after, active, call)) in
        cases.into_iter().enumerate()
    {
        let mut state = State {
            bytes: [0, 0, 0, 0, count, 0x91, 0xa2, 0xb3, latch],
        };
        state.bytes[..4].copy_from_slice(&reasons);
        let mut expected = state.bytes;
        expected[4] = after;
        expected[8] = active;
        let mut calls = Vec::new();
        let callback = |event, live: &mut State| {
            // Count store precedes callback; latch store follows it.
            assert_eq!(live.bytes[4], after, "case{index}");
            assert_eq!(live.bytes[8], latch, "case{index}");
            calls.push(event);
            Ok(())
        };
        if gain {
            state.focus_gained(blocked, callback)
        } else {
            state.focus_lost(blocked, callback)
        }
        .unwrap();
        assert_eq!(state.bytes, expected, "case{index}");
        assert_eq!(calls, call.into_iter().collect::<Vec<_>>(), "case{index}");
    }
}

#[test]
fn native_focus_changes_only_enabled_bytes_of_mask2_rows() {
    let mut rows = [0u32, 1, 2, 3, 4, 10].map(|mask| {
        let mut task = [0x95; 20];
        task[0] = 7;
        task[4..8].copy_from_slice(&mask.to_le_bytes());
        task
    });
    let original = rows;
    let mut state = State::default();
    state
        .focus_lost(false, |event, _| {
            event.apply(&mut rows);
            Ok(())
        })
        .unwrap();
    for (index, enabled) in [7, 7, 0, 0, 7, 0].into_iter().enumerate() {
        let mut expected = original[index];
        expected[0] = enabled;
        assert_eq!(rows[index], expected);
    }
    state
        .focus_gained(false, |event, _| {
            event.apply(&mut rows);
            Ok(())
        })
        .unwrap();
    for (index, enabled) in [7, 7, 1, 1, 7, 1].into_iter().enumerate() {
        let mut expected = original[index];
        expected[0] = enabled;
        assert_eq!(rows[index], expected);
    }
}

#[test]
fn focus_callback_failure_retains_reached_count_without_latch_store() {
    let mut state = State::default();
    assert_eq!(
        state.focus_lost(false, |event, live| {
            assert_eq!(event, Timers::Suspend);
            assert_eq!(live.bytes, [0, 0, 0, 0, 1, 0, 0, 0, 0]);
            Err("suspend failed".into())
        }),
        Err("suspend failed".into())
    );
    assert_eq!(state.bytes, [0, 0, 0, 0, 1, 0, 0, 0, 0]);
    state.bytes[8] = 1;
    assert_eq!(
        state.focus_gained(false, |event, live| {
            assert_eq!(event, Timers::Resume);
            assert_eq!(live.bytes, [0, 0, 0, 0, 0, 0, 0, 0, 1]);
            Err("resume failed".into())
        }),
        Err("resume failed".into())
    );
    assert_eq!(state.bytes, [0, 0, 0, 0, 0, 0, 0, 0, 1]);
}

#[test]
fn native_ordinary_unpause_can_resume_while_focus_remains_lost() {
    let mut state = State::default();
    let mut calls = Vec::new();
    state
        .pause(0, |event, _| {
            calls.push(event);
            Ok(())
        })
        .unwrap();
    state
        .focus_lost(false, |_, _| panic!("shared latch already set"))
        .unwrap();
    assert_eq!(state.bytes, [1, 0, 0, 0, 1, 0, 0, 0, 1]);
    state
        .unpause(0, |event, _| {
            calls.push(event);
            Ok(())
        })
        .unwrap();
    assert_eq!(state.bytes, [0, 0, 0, 0, 1, 0, 0, 0, 0]);
    assert_eq!(calls, [Timers::Suspend, Timers::Resume]);
    state
        .focus_gained(false, |_, _| panic!("native latch already clear"))
        .unwrap();
    assert_eq!(state.bytes, [0; 9]);
}
