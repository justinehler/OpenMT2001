//! Frontend guards004ad610/004ad640. The callback is the actual audio
//! frontend slot26/27; changing this byte alone does not perform its effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frontend {
    suspended: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Suspend,
    Resume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioOutcome {
    Applied,
    GuardedInactive,
}

/// Result of the whole focus-change prefix the session worker runs for one
/// focus event: either no UI receiver was attached, or it completed, with the
/// audio frontend outcome when the event reached it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefix {
    ReceiverAbsent,
    Complete(Option<AudioOutcome>),
}

impl Frontend {
    /// Supply the real process byte8af401. This is not a per-World default.
    pub fn from_process_byte(suspended: u8) -> Self {
        Self { suspended }
    }
    pub fn suspended_byte(&self) -> u8 {
        self.suspended
    }

    pub fn apply(
        &mut self,
        focused: bool,
        target_present: bool,
        reason3: i8,
        mut callback: impl FnMut(Action) -> Result<AudioOutcome, String>,
    ) -> Result<Option<AudioOutcome>, String> {
        if !target_present {
            return Ok(None);
        }
        if !focused && self.suspended == 0 {
            let outcome = callback(Action::Suspend)?;
            self.suspended = 1;
            return Ok(Some(outcome));
        } else if focused && self.suspended != 0 && reason3 <= 0 {
            let outcome = callback(Action::Resume)?;
            self.suspended = 0;
            return Ok(Some(outcome));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_target_flag_and_signed_reason3_guards() {
        for (focused, target, flag, reason, after, action) in [
            (false, false, 0, 0, 0, None),
            (true, false, 1, 0, 1, None),
            (false, true, 0, 1, 1, Some(Action::Suspend)),
            (false, true, 1, 0, 1, None),
            (true, true, 0, 0, 0, None),
            (true, true, 1, 1, 1, None),
            (true, true, 0x80, 127, 0x80, None),
            (true, true, 1, 0, 0, Some(Action::Resume)),
            (true, true, 0x80, -128, 0, Some(Action::Resume)),
            (true, true, 1, -1, 0, Some(Action::Resume)),
        ] {
            let mut state = Frontend::from_process_byte(flag);
            let mut calls = Vec::new();
            state
                .apply(focused, target, reason, |call| {
                    calls.push(call);
                    Ok(AudioOutcome::Applied)
                })
                .unwrap();
            assert_eq!(state.suspended_byte(), after);
            assert_eq!(calls, action.into_iter().collect::<Vec<_>>());
        }
    }
    #[test]
    fn incomplete_audio_callback_does_not_publish_flag_transition() {
        let mut state = Frontend::from_process_byte(0);
        assert!(
            state
                .apply(false, true, 0, |_| Err("actual audio unavailable".into()))
                .is_err()
        );
        assert_eq!(state.suspended_byte(), 0);
        state = Frontend::from_process_byte(1);
        assert!(
            state
                .apply(true, true, 0, |_| Err("actual audio unavailable".into()))
                .is_err()
        );
        assert_eq!(state.suspended_byte(), 1);
    }
    #[test]
    fn authoritative_inactive_audio_still_completes_the_native_wrapper_latch() {
        let mut state = Frontend::from_process_byte(0);
        assert_eq!(
            state
                .apply(false, true, 0, |_| Ok(AudioOutcome::GuardedInactive))
                .unwrap(),
            Some(AudioOutcome::GuardedInactive)
        );
        assert_eq!(state.suspended_byte(), 1);
        assert_eq!(
            state
                .apply(true, true, 0, |_| Ok(AudioOutcome::GuardedInactive))
                .unwrap(),
            Some(AudioOutcome::GuardedInactive)
        );
        assert_eq!(state.suspended_byte(), 0);
    }
}
