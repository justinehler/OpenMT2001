//! New-press filtering for the city's logical key shortcuts.
//!
//! The original reads DirectInput scan state at its 50 Hz input poll and acts
//! on a rising edge: down now, up in the previous poll (0x4e51c0). A held key
//! therefore acts once, and Windows key auto-repeat never reaches these checks.
//! Escape (scan 0x01) is one of those edge tests (0x4ba6bc). Window key events
//! also carry OS auto-repeat and focus-reconciliation presses, so they are
//! filtered here before the city shortcuts see them.
use winit::keyboard::NamedKey;

/// Escape's physical history across window key events.
#[derive(Default)]
pub(super) struct Input {
    escape_held: bool,
}
impl Input {
    /// Whether this key event should reach the city shortcuts. Releases always
    /// pass so held state can retire. An Escape press passes only when it is
    /// a new physical press: not an OS repeat, not a synthetic focus-gain
    /// press, and not a second down without an up in between.
    pub(super) fn admit(
        &mut self,
        key: NamedKey,
        pressed: bool,
        repeat: bool,
        synthetic: bool,
    ) -> bool {
        if key != NamedKey::Escape {
            return true;
        }
        let edge = pressed && !repeat && !synthetic && !self.escape_held;
        if !repeat {
            self.escape_held = pressed;
        }
        !pressed || edge
    }
    /// The window lost focus. A key released elsewhere sends no up event, so
    /// forget it rather than ignore the next real press.
    pub(super) fn unfocus(&mut self) {
        self.escape_held = false;
    }
}

/// What a key press does while In-Game Options is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OptionsKey {
    /// Leave the settings page for the Options list.
    Back,
    /// Hide the Options panel and resume.
    Close,
}

/// Only Escape acts on In-Game Options: its new press calls the menu
/// dispatcher 0x491720, which hides an open resume panel. The original has no
/// Return binding for the panel. The only readers of main Return (scan 0x1c)
/// are the generic prompt 0x42a1f0, the debug console 0x434bc0 and the
/// sequence skip 0x4b1980, and keypad Enter (scan 0x9c) is read only by the
/// sequence skip.
pub(super) fn options_key(key: NamedKey, settings_open: bool) -> Option<OptionsKey> {
    match key {
        NamedKey::Escape if settings_open => Some(OptionsKey::Back),
        NamedKey::Escape => Some(OptionsKey::Close),
        _ => None,
    }
}

/// Where an admitted key press goes while In-Game Options or a generic
/// prompt is shown in a live city.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModalKey {
    /// In-Game Options is shown and owns the key.
    Options(OptionsKey),
    /// Escape opens In-Game Options over the prompt.
    OpenOptions,
    /// The prompt keeps the key; its Return and Backspace are read from the
    /// physical key history instead.
    Consumed,
    /// Neither is shown: the city's other key handlers run.
    Pass,
}

/// Whether Escape may call the menu dispatcher 0x491720. The central
/// callback's Escape test (0x4ba6bc..0x4ba6f8) reads only the new press of
/// scan 0x01, the pause latch byte 0x8b22d8 and UI mode 4. Nothing in that
/// test or in 0x491720 looks at the prompt queue, so a portrait prompt, a
/// choice prompt and a card notice all let Escape through. A held pause
/// reason (F1/P, a script's Game.Pause, Options itself) sets 0x8b22d8 and
/// stops it.
pub(super) fn escape_opens_options(pause_active: bool) -> bool {
    !pause_active
}

/// Route a key press past an open Options panel, then past a prompt that
/// holds the controls. Options is checked first: once it is open over a
/// prompt, the prompt underneath gets no keys until Continue.
pub(super) fn modal_key(
    key: NamedKey,
    options_open: bool,
    settings_open: bool,
    prompt_open: bool,
    pause_active: bool,
) -> ModalKey {
    if options_open {
        return options_key(key, settings_open).map_or(ModalKey::Consumed, ModalKey::Options);
    }
    if !prompt_open {
        return ModalKey::Pass;
    }
    if key == NamedKey::Escape && escape_opens_options(pause_active) {
        ModalKey::OpenOptions
    } else {
        ModalKey::Consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_opens_options_over_a_prompt_once_per_press() {
        let mut input = Input::default();
        let mut options_open = false;
        let mut opened = 0;
        let escape = NamedKey::Escape;
        // A held Escape over a prompt, then its release.
        for (pressed, repeat) in [(true, false), (true, true), (true, false), (false, false)] {
            if !input.admit(escape, pressed, repeat, false) || !pressed {
                continue;
            }
            match modal_key(escape, options_open, false, true, false) {
                ModalKey::OpenOptions => {
                    options_open = true;
                    opened += 1;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(opened, 1, "a held Escape opens Options once");
        // With Options open over the prompt, Options owns every key, and the
        // prompt's own Return never reaches it.
        assert_eq!(
            modal_key(NamedKey::Enter, true, false, true, true),
            ModalKey::Consumed
        );
        assert_eq!(
            modal_key(NamedKey::Backspace, true, false, true, true),
            ModalKey::Consumed
        );
        assert_eq!(
            modal_key(escape, true, false, true, true),
            ModalKey::Options(OptionsKey::Close)
        );
        assert_eq!(
            modal_key(escape, true, true, true, true),
            ModalKey::Options(OptionsKey::Back)
        );
    }

    #[test]
    fn a_held_pause_reason_stops_escape_but_not_the_prompt_keys() {
        // 0x8b22d8 set: Escape over a prompt does nothing.
        assert_eq!(
            modal_key(NamedKey::Escape, false, false, true, true),
            ModalKey::Consumed
        );
        assert!(!escape_opens_options(true));
        assert!(escape_opens_options(false));
        // Other keys stay with the prompt whatever the pause state.
        for pause_active in [false, true] {
            for key in [NamedKey::Enter, NamedKey::ArrowLeft, NamedKey::Tab] {
                assert_eq!(
                    modal_key(key, false, false, true, pause_active),
                    ModalKey::Consumed
                );
            }
        }
        // Without a prompt or Options the city's own handlers run.
        assert_eq!(
            modal_key(NamedKey::Escape, false, false, false, false),
            ModalKey::Pass
        );
    }

    /// One window key event: logical key, pressed, OS repeat, synthetic.
    type Event = (NamedKey, bool, bool, bool);

    /// Feed events through the filter and count what Options would do.
    fn options_actions(events: &[Event], settings_open: bool) -> Vec<OptionsKey> {
        let mut input = Input::default();
        events
            .iter()
            .filter(|&&(key, pressed, repeat, synthetic)| {
                input.admit(key, pressed, repeat, synthetic) && pressed
            })
            .filter_map(|&(key, ..)| options_key(key, settings_open))
            .collect()
    }

    #[test]
    fn held_escape_closes_options_once() {
        let escape = NamedKey::Escape;
        // A held key: one press, OS auto-repeat, and a duplicate down without
        // an up (as some platforms send), then the release.
        let held = [
            (escape, true, false, false),
            (escape, true, true, false),
            (escape, true, true, false),
            (escape, true, false, false),
            (escape, true, true, false),
            (escape, false, false, false),
        ];
        for settings_open in [false, true] {
            let expected = if settings_open {
                OptionsKey::Back
            } else {
                OptionsKey::Close
            };
            assert_eq!(options_actions(&held, settings_open), vec![expected]);
        }
        // A second physical press after the release acts again.
        let twice = [held.as_slice(), held.as_slice()].concat();
        assert_eq!(options_actions(&twice, false).len(), 2);
    }

    #[test]
    fn return_and_keypad_enter_do_nothing_to_options() {
        // Winit reports main Return and keypad Enter alike as logical Enter.
        let enter = [
            (NamedKey::Enter, true, false, false),
            (NamedKey::Enter, true, true, false),
            (NamedKey::Enter, false, false, false),
        ];
        for settings_open in [false, true] {
            assert!(options_actions(&enter, settings_open).is_empty());
        }
    }

    #[test]
    fn focus_presses_do_not_act_and_unfocus_forgets_a_held_escape() {
        let mut input = Input::default();
        // Focus gain while Escape is held reports a synthetic press.
        assert!(!input.admit(NamedKey::Escape, true, false, true));
        assert!(!input.admit(NamedKey::Escape, true, true, false));
        assert!(input.admit(NamedKey::Escape, false, false, false));
        assert!(input.admit(NamedKey::Escape, true, false, false));
        // Released while another window had focus: no up event arrives.
        input.unfocus();
        assert!(input.admit(NamedKey::Escape, true, false, false));
    }

    #[test]
    fn other_keys_pass_unchanged() {
        let mut input = Input::default();
        for key in [NamedKey::ArrowLeft, NamedKey::Enter, NamedKey::PageUp] {
            assert!(input.admit(key, true, false, false));
            assert!(input.admit(key, true, true, false));
            assert!(input.admit(key, false, false, false));
        }
    }
}
