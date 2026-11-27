//! Foreground view shortcuts. Numeric camera callbacks stay with their owner.
use super::process_frontend::PhysicalSample;
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    CityView,
    ParentBlock,
}

#[derive(Default, Clone, Copy)]
pub(super) struct Gates {
    pub active: bool,
    pub modal: bool,
    pub tutorial: bool,
    pub text_entry: bool,
    pub city_view: bool,
}

#[derive(Default)]
pub(super) struct Input {
    held: [bool; 2],
    left_alt: bool,
}
impl Input {
    pub(super) fn sample(&mut self, sample: PhysicalSample, gates: Gates) -> Option<Action> {
        let (key, down, fresh) = match sample {
            PhysicalSample::Key { key, down, repeat } => (key, down, !repeat),
            // Focus reconciliation updates history without inventing a press.
            PhysicalSample::SyntheticKey { key, down } => (key, down, false),
            PhysicalSample::Unfocus => {
                *self = Self::default();
                return None;
            }
            _ => return None,
        };
        if key == PhysicalKey::Code(KeyCode::AltLeft) {
            self.left_alt = down;
            return None;
        }
        // 4ba643 uses the Tab edge and excludes DIK_LMENU (left Alt).
        let (index, action) = match key {
            PhysicalKey::Code(KeyCode::Tab) => (0, Action::CityView),
            PhysicalKey::Code(KeyCode::Home) => (1, Action::ParentBlock),
            _ => return None,
        };
        let edge = fresh && down && !self.held[index];
        // Always retire physical history, including events swallowed by a gate.
        self.held[index] = down;
        (edge
            && gates.active
            && !gates.modal
            && !gates.tutorial
            && !gates.text_entry
            && (action != Action::CityView || gates.city_view && !self.left_alt))
            .then_some(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode, down: bool, repeat: bool) -> PhysicalSample {
        PhysicalSample::Key {
            key: PhysicalKey::Code(code),
            down,
            repeat,
        }
    }
    fn open() -> Gates {
        Gates {
            active: true,
            city_view: true,
            ..Gates::default()
        }
    }
    #[test]
    fn original_view_keys_map_once_per_press() {
        let mut input = Input::default();
        for (code, action) in [
            (KeyCode::Tab, Action::CityView),
            (KeyCode::Home, Action::ParentBlock),
        ] {
            assert_eq!(input.sample(key(code, true, false), open()), Some(action));
            assert_eq!(input.sample(key(code, true, true), open()), None);
            assert_eq!(input.sample(key(code, true, false), open()), None);
            assert_eq!(input.sample(key(code, false, false), open()), None);
            assert_eq!(input.sample(key(code, true, false), open()), Some(action));
            input.sample(key(code, false, false), open());
        }
    }
    #[test]
    fn gates_consume_presses_without_replaying_after_dismissal() {
        for gate in [
            Gates {
                active: false,
                ..open()
            },
            Gates {
                modal: true,
                ..open()
            },
            Gates {
                tutorial: true,
                ..open()
            },
            Gates {
                text_entry: true,
                ..open()
            },
        ] {
            for code in [KeyCode::Tab, KeyCode::Home] {
                let mut input = Input::default();
                assert_eq!(input.sample(key(code, true, false), gate), None);
                assert_eq!(input.sample(key(code, true, true), open()), None);
                assert_eq!(input.sample(key(code, true, false), open()), None);
                input.sample(key(code, false, false), gate);
                assert!(input.sample(key(code, true, false), open()).is_some());
            }
        }
    }
    #[test]
    fn alt_tab_and_disabled_city_view_do_not_toggle() {
        {
            let alt = KeyCode::AltLeft;
            let mut input = Input::default();
            input.sample(key(alt, true, false), open());
            assert_eq!(input.sample(key(KeyCode::Tab, true, false), open()), None);
            input.sample(key(alt, false, false), open());
            assert_eq!(input.sample(key(KeyCode::Tab, true, true), open()), None);
            input.sample(key(KeyCode::Tab, false, false), open());
            assert_eq!(
                input.sample(key(KeyCode::Tab, true, false), open()),
                Some(Action::CityView)
            );
        }
        assert_eq!(
            Input::default().sample(
                key(KeyCode::Tab, true, false),
                Gates {
                    city_view: false,
                    ..open()
                }
            ),
            None
        );
    }
    #[test]
    fn focus_reconciliation_never_fabricates_a_view_action() {
        for code in [KeyCode::Tab, KeyCode::Home] {
            let mut input = Input::default();
            input.sample(key(code, true, false), open());
            input.sample(PhysicalSample::Unfocus, open());
            assert_eq!(
                input.sample(
                    PhysicalSample::SyntheticKey {
                        key: PhysicalKey::Code(code),
                        down: true
                    },
                    open()
                ),
                None
            );
            assert_eq!(input.sample(key(code, true, true), open()), None);
            input.sample(key(code, false, false), open());
            assert!(input.sample(key(code, true, false), open()).is_some());
        }
    }
}
