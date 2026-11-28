use super::*;

fn legacy_dispatch(timers: &[openmt2001::timers::Task]) -> Vec<u8> {
    let mut saved = Output(vec![3, 1, 0, 1, 1, 0xa5, 0x5a, 0xee, 1]);
    saved.word(17_f32.to_bits());
    saved.word(123);
    saved.word(456);
    saved.blob(&flat(timers)).unwrap();
    saved.0
}

#[test]
fn legacy_dispatch_keeps_current_pause_and_timer_effects() {
    use openmt2001::{pause, process_dispatch::DispatchCustody, timers};
    let mut old_timers = timers::startup_tasks([0xcc; 20]);
    pause::Timers::Suspend.apply(&mut old_timers);
    let saved = legacy_dispatch(&old_timers);
    for current_owner in [None, Some(0), Some(1), Some(3), Some(4)] {
        let mut current = DispatchCustody::startup(pause::State::default()).unwrap();
        if let Some(reason) = current_owner {
            let mut apply = |change: pause::Timers, _: &mut pause::State| {
                change.apply(&mut current.tasks);
                Ok(())
            };
            if reason == 4 {
                current.pause.focus_lost(false, &mut apply).unwrap();
            } else {
                current.pause.pause(reason, &mut apply).unwrap();
            }
        }
        // The native load completion releases only its startup hold.
        current
            .pause
            .unpause(0, |change, _| {
                change.apply(&mut current.tasks);
                Ok(())
            })
            .unwrap();
        let expected_pause = current.pause.clone();
        let expected_tasks = current.tasks.clone();
        let mut input = Input::new(&saved);
        skip_saved_dispatch(&mut input).unwrap();
        input.end().unwrap();
        assert_eq!(current.pause, expected_pause);
        assert_eq!(current.tasks, expected_tasks);
        let mut ticks = 0;
        timers::dispatch(&mut current.tasks, 40, |_, callback, _| {
            ticks += u32::from(callback == 0x481680);
            Ok(())
        })
        .unwrap();
        // Native unpause0 can resume with a focus count still present. Keep
        // that asymmetry too; ordinary reasons continue to suspend logic.
        assert_eq!(ticks, u32::from(matches!(current_owner, None | Some(4))));
    }
}

#[test]
fn legacy_dispatch_layout_keeps_following_fields_and_validates_timer_blob() {
    for count in [0, 4, 1000] {
        let mut saved = legacy_dispatch(&vec![[0; 20]; count]);
        saved.extend_from_slice(&0x1234_5678_u32.to_le_bytes());
        let mut input = Input::new(&saved);
        skip_saved_dispatch(&mut input).unwrap();
        assert_eq!(input.word().unwrap(), 0x1234_5678);
        input.end().unwrap();
    }
    for bytes in [vec![0; 20], legacy_dispatch(&vec![[0; 20]; 1001])] {
        assert!(skip_saved_dispatch(&mut Input::new(&bytes)).is_err());
    }
    let mut saved = legacy_dispatch(&[[0; 20]]);
    saved.pop();
    assert!(skip_saved_dispatch(&mut Input::new(&saved)).is_err());
    let mut saved = legacy_dispatch(&[]);
    saved[21..25].copy_from_slice(&1_u32.to_le_bytes());
    saved.push(0);
    assert_eq!(
        skip_saved_dispatch(&mut Input::new(&saved)),
        Err("V2 timer pool size invalid".into())
    );
}
