use super::*;
use crate::{human_dialog::Dialog, intro::process_frontend::ProcessFrontend, prompt};
use winit::window::WindowId;

struct Harness {
    process: super::super::process_frontend::Owner,
    input: Input,
    prompts: prompt::State,
}
impl Harness {
    fn new() -> Self {
        let (process, receipt) = crate::live_host::process_runtime::test_frontend_startup();
        let process = ProcessFrontend::new(
            receipt,
            process,
            WindowId::from(7),
            crate::settings_menu::Levels::default(),
        )
        .unwrap();
        process.borrow_mut().observe(true).unwrap();
        Self {
            process,
            input: Input::default(),
            prompts: prompt::State::default(),
        }
    }
    fn physical(&mut self, sample: PhysicalSample) {
        self.process
            .borrow_mut()
            .sample(WindowId::from(7), sample, 0, 0)
            .unwrap();
        for sample in self.process.borrow_mut().take_samples() {
            self.input.physical(sample);
        }
    }
    fn key(&mut self, code: KeyCode, down: bool, repeat: bool) {
        self.physical(PhysicalSample::Key {
            key: PhysicalKey::Code(code),
            down,
            repeat,
        });
    }
    fn event1(&mut self, active: bool) -> Option<u32> {
        self.input.event1(active.then_some(&mut self.prompts))
    }
    fn open(&mut self, button_count: usize) {
        self.prompts.submit(request(button_count, 17));
        self.event1(true); // The first UI event arms this record.
    }
    fn no_reply(&mut self) {
        assert!(self.prompts.take_dialog_replies().is_empty());
    }
}
fn request(button_count: usize, context: u32) -> prompt::Request {
    prompt::Request {
        script: None,
        dialog: Some(Dialog {
            buttons: [1636, 1637],
            button_count,
            callback: 0x484d60,
            context,
            timeout: false,
            handler_written: true,
            context_written: true,
            ..Default::default()
        }),
        title: 0,
        body: 0,
        dismissal_id: 0,
        card: None,
    }
}

#[test]
fn fresh_preview_worker_script_prompt_uses_dialog_reply() {
    use openmt2001::lua4::Value;

    for (code, button) in [(KeyCode::Enter, 0), (KeyCode::Backspace, 1)] {
        let mut h = Harness::new();
        let request = crate::live_host::script_request(
            &[Value::Number(1.), Value::Number(2.), Value::Number(1.)],
            false,
            false,
        )
        .unwrap();
        assert_eq!(request.dialog.unwrap().button_count, 0);
        h.prompts.submit(request);
        assert_eq!(h.prompts.begin_dismiss(), None);
        h.event1(true);
        h.key(code, true, false);
        h.event1(true);
        assert_eq!(
            h.prompts.take_dialog_replies(),
            vec![crate::human_dialog::Reply {
                callback: 0,
                context: 1,
                button,
            }]
        );
        assert!(h.prompts.request().is_some(), "callback precedes unlink");
        h.prompts.finish_dismiss(vec![]);
        assert!(h.prompts.request().is_none());
        h.event1(true);
        h.no_reply();
    }
}

#[test]
fn options_quit_menu_backspace_and_return_use_original_replies() {
    for (code, button) in [(KeyCode::Backspace, 1), (KeyCode::Enter, 0)] {
        let mut h = Harness::new();
        let assets = crate::game_options::Assets::test_menu();
        let mut menu = crate::game_options::View::default();
        menu.open(true);
        assert_eq!(menu.mouse(&assets, Some([65., 5.]), true), None);
        assert_eq!(
            menu.mouse(&assets, Some([65., 5.]), false),
            Some(crate::game_options::Button::Quit)
        );
        h.prompts = std::mem::take(&mut menu.quit_prompt);
        h.event1(true);
        h.key(code, true, false);
        h.event1(true);
        assert_eq!(
            h.prompts.take_dialog_replies(),
            vec![crate::human_dialog::Reply {
                callback: openmt2001::human_dialog::quit::CITY_CALLBACK,
                context: 0,
                button,
            }]
        );
        h.prompts.finish_dismiss(vec![]);
        assert!(h.prompts.request().is_none());
        assert!(menu.open && menu.resume_running);
    }
}

#[test]
fn production_dialog_keys_return_and_backspace_match_mouse_replies() {
    for (code, button) in [(KeyCode::Enter, 0), (KeyCode::Backspace, 1)] {
        let mut h = Harness::new();
        h.open(2);
        h.key(code, true, false);
        h.event1(true);
        let keyboard = h.prompts.take_dialog_replies();
        let mut mouse = prompt::State::default();
        mouse.submit(request(2, 17));
        let buttons = [[0., 0., 60., 24.], [80., 0., 140., 24.]];
        mouse.cursor([buttons[button][0] + 1., 1.]);
        mouse.dialog_mouse(true, &buttons);
        mouse.dialog_mouse(false, &buttons);
        assert_eq!(keyboard, mouse.take_dialog_replies());
        assert_eq!(
            keyboard,
            vec![crate::human_dialog::Reply {
                callback: 0x484d60,
                context: 17,
                button: button as u32,
            }]
        );
        assert!(h.prompts.request().is_some(), "callback precedes unlink");
        h.prompts.finish_dismiss(vec![]);
        assert!(h.prompts.request().is_none());
    }
}

#[test]
fn production_dialog_keys_first_tick_held_repeat_and_replacement() {
    let mut h = Harness::new();
    h.key(KeyCode::Enter, true, false);
    h.open(2); // Opening with Return already held only arms.
    h.event1(true);
    h.no_reply();
    h.key(KeyCode::Enter, false, false);
    h.event1(true);
    h.key(KeyCode::Enter, true, false);
    h.event1(true);
    assert_eq!(h.prompts.take_dialog_replies().len(), 1);
    // Both a repeated OS event and a duplicate non-repeat keydown are inert,
    // including while the worker reply is pending and after its replacement.
    for _ in 0..3 {
        h.key(KeyCode::Enter, true, true);
        h.key(KeyCode::Enter, true, false);
        h.event1(true);
        h.no_reply();
    }
    h.prompts.finish_dismiss(vec![request(2, 29)]);
    h.event1(true);
    h.key(KeyCode::Enter, true, true);
    h.event1(true);
    h.no_reply();
    h.key(KeyCode::Enter, false, false);
    h.event1(true);
    h.key(KeyCode::Enter, true, false);
    h.event1(true);
    let replies = h.prompts.take_dialog_replies();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].context, 29);
}

#[test]
fn production_dialog_keys_no_dialog_inactive_and_button_count() {
    for code in [KeyCode::Enter, KeyCode::Backspace] {
        let mut h = Harness::new();
        h.key(code, true, false);
        h.event1(true); // No request: consume, never defer the edge.
        h.open(2);
        h.event1(true);
        h.no_reply();
        h.key(code, false, false);
        h.event1(true);
        h.key(code, true, false);
        h.event1(false); // Save/name entry or another owner has priority.
        h.event1(true);
        h.no_reply();
    }
    for count in 0..=2 {
        for (code, button) in [(KeyCode::Enter, 0), (KeyCode::Backspace, 1)] {
            let mut h = Harness::new();
            h.open(count);
            h.key(code, true, false);
            h.event1(true);
            assert_eq!(h.prompts.take_dialog_replies()[0].button, button);
            h.prompts.finish_dismiss(vec![]);
            assert!(h.prompts.request().is_none());
        }
    }
}

#[test]
fn production_dialog_keys_focus_changes_do_not_repeat_a_choice() {
    let mut h = Harness::new();
    h.open(2);
    h.key(KeyCode::Backspace, true, false);
    h.event1(true);
    assert_eq!(h.prompts.take_dialog_replies().len(), 1);
    h.prompts.finish_dismiss(vec![request(2, 29)]);
    h.input.focus(false);
    h.process.borrow_mut().observe(false).unwrap();
    h.physical(PhysicalSample::Unfocus);
    h.event1(true);
    h.process.borrow_mut().observe(true).unwrap();
    h.input.focus(true);
    // Winit may synthesize another non-repeat down on focus gain.
    h.physical(PhysicalSample::SyntheticKey {
        key: PhysicalKey::Code(KeyCode::Backspace),
        down: true,
    });
    h.event1(true);
    h.event1(true);
    h.no_reply();
    h.key(KeyCode::Backspace, false, false);
    h.event1(true);
    h.key(KeyCode::Backspace, true, false);
    h.event1(true);
    assert_eq!(h.prompts.take_dialog_replies()[0].context, 29);
}

#[test]
fn production_dialog_keys_focus_reconciliation_never_creates_a_press() {
    for code in [KeyCode::Enter, KeyCode::Backspace] {
        let mut h = Harness::new();
        h.open(2);
        h.input.focus(false);
        h.physical(PhysicalSample::Unfocus);
        h.input.focus(true);
        h.physical(PhysicalSample::SyntheticKey {
            key: PhysicalKey::Code(code),
            down: true,
        });
        h.event1(true);
        h.no_reply();
        h.key(code, true, true);
        h.event1(true);
        h.no_reply();
        h.key(code, false, false);
        h.event1(true);
        h.key(code, true, false);
        h.event1(true);
        assert_eq!(h.prompts.take_dialog_replies().len(), 1);
    }
    let mut h = Harness::new();
    h.open(2);
    h.key(KeyCode::Enter, true, false);
    h.input.focus(false); // Lose focus before this edge can be sampled.
    h.physical(PhysicalSample::Unfocus);
    h.event1(true);
    h.no_reply();
    h.input.focus(true); // Released in the background: no synthetic down.
    h.key(KeyCode::Enter, true, false);
    h.event1(true);
    assert_eq!(h.prompts.take_dialog_replies().len(), 1);
}

#[test]
fn production_dialog_keys_held_press_cannot_answer_full_queue_replacement() {
    let mut h = Harness::new();
    for context in 1..=4 {
        h.prompts.submit(request(2, context));
    }
    for _ in 0..3 {
        h.prompts.dismiss_with(|_, _| {});
    }
    h.event1(true); // Arm the last-created slot, now at the head.
    // Fill the other slots so the next submit overwrites the new tail.
    for context in 5..=7 {
        h.prompts.submit(request(2, context));
    }
    h.prompts.submit(request(2, 8));
    h.key(KeyCode::Enter, true, false);
    h.event1(true);
    assert_eq!(h.prompts.take_dialog_replies()[0].context, 4);
    h.prompts.finish_dismiss(vec![]);
    for _ in 0..2 {
        h.prompts.dismiss_with(|_, _| {});
    }
    h.event1(true);
    h.event1(true);
    h.no_reply(); // Held Return cannot answer the overwritten tail.
    h.key(KeyCode::Enter, false, false);
    h.event1(true);
    h.key(KeyCode::Enter, true, false);
    h.event1(true);
    assert_eq!(h.prompts.take_dialog_replies()[0].context, 8);
}

#[test]
fn production_dialog_keys_simultaneous_edges_resume_after_callback_and_close() {
    let mut h = Harness::new();
    h.open(1);
    h.prompts.submit(request(0, 29));
    h.key(KeyCode::Enter, true, false);
    h.key(KeyCode::Backspace, true, false);
    h.event1(true);
    let replies = h.prompts.take_dialog_replies();
    assert_eq!(replies.len(), 1);
    assert_eq!((replies[0].button, replies[0].context), (0, 17));
    assert_eq!(h.prompts.request().unwrap().dialog.unwrap().context, 17);
    // The callback appends a third request while the first slot is still active.
    h.prompts.finish_dismiss(vec![request(2, 41)]);
    let replies = h.prompts.take_dialog_replies();
    assert_eq!(replies.len(), 1);
    assert_eq!((replies[0].button, replies[0].context), (1, 29));
    assert_eq!(h.prompts.request().unwrap().dialog.unwrap().context, 29);
    h.prompts.finish_dismiss(vec![]);
    assert_eq!(h.prompts.request().unwrap().dialog.unwrap().context, 41);
    h.event1(true);
    h.event1(true);
    h.no_reply(); // Neither held edge is reused against the third request.
}

fn notice(dismissal_id: u32) -> prompt::Request {
    prompt::Request {
        dialog: None,
        dismissal_id,
        ..request(0, 0)
    }
}

#[test]
fn production_dialog_keys_script_and_unrelated_keys_keep_independent_routes() {
    let mut h = Harness::new();
    h.prompts.submit(notice(42));
    h.key(KeyCode::Backspace, true, false);
    assert_eq!(h.event1(true), None); // The first event only arms.
    h.key(KeyCode::Enter, true, false);
    assert_eq!(h.event1(true), Some(42)); // Main Return dismisses the notice.
    h.no_reply();
    assert!(
        h.prompts.request().is_some(),
        "script callback precedes unlink"
    );
    h.prompts.finish_dismiss(vec![request(2, 17)]);
    h.event1(true);
    for code in [
        KeyCode::NumpadEnter,
        KeyCode::Escape,
        KeyCode::Space,
        KeyCode::F1,
        KeyCode::KeyP,
    ] {
        h.key(code, true, false);
        h.event1(true);
    }
    h.no_reply();
}

#[test]
fn held_return_dismisses_one_notice_per_press() {
    let mut h = Harness::new();
    for id in [41, 42, 43] {
        h.prompts.submit(notice(id));
    }
    assert_eq!(h.event1(true), None);
    h.key(KeyCode::Enter, true, false);
    assert_eq!(h.event1(true), Some(41));
    // Held: OS auto-repeat and a duplicate down without a release, before
    // and after the worker finishes the first dismissal.
    for _ in 0..3 {
        h.key(KeyCode::Enter, true, true);
        h.key(KeyCode::Enter, true, false);
        assert_eq!(h.event1(true), None);
    }
    h.prompts.finish_dismiss(vec![]);
    for _ in 0..5 {
        h.key(KeyCode::Enter, true, true);
        h.key(KeyCode::Enter, true, false);
        assert_eq!(h.event1(true), None);
    }
    assert_eq!(h.prompts.request().unwrap().dismissal_id, 42);
    // The next physical press dismisses the next notice.
    h.key(KeyCode::Enter, false, false);
    assert_eq!(h.event1(true), None);
    h.key(KeyCode::Enter, true, false);
    assert_eq!(h.event1(true), Some(42));
}

#[test]
fn held_return_dismisses_one_card_notice_per_press() {
    // Card notices have no script callback and dismiss on the spot.
    let mut h = Harness::new();
    for _ in 0..3 {
        h.prompts.submit(notice(0));
    }
    h.event1(true);
    h.key(KeyCode::Enter, true, false);
    for _ in 0..10 {
        assert_eq!(h.event1(true), None);
        h.key(KeyCode::Enter, true, true);
    }
    let remaining = h.prompts.slots.iter().filter(|s| s.active).count();
    assert_eq!(remaining, 2, "one press dismisses exactly one notice");
}

#[test]
fn keypad_enter_does_not_dismiss_a_notice_or_answer_a_dialog() {
    let mut h = Harness::new();
    h.prompts.submit(notice(42));
    h.event1(true);
    for repeat in [false, true, true] {
        h.key(KeyCode::NumpadEnter, true, repeat);
        assert_eq!(h.event1(true), None);
    }
    h.key(KeyCode::NumpadEnter, false, false);
    assert_eq!(h.event1(true), None);
    assert_eq!(h.prompts.request().unwrap().dismissal_id, 42);
    let mut h = Harness::new();
    h.open(2);
    h.key(KeyCode::NumpadEnter, true, false);
    h.event1(true);
    h.no_reply();
}
