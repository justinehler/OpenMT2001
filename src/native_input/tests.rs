//! Synthetic packet-order and failure-boundary regressions.
use super::*;

fn mouse(dx: i32, dy: i32, wheel: i32, buttons: [u8; 4]) -> MousePacket {
    let mut bytes = [0; 16];
    bytes[0..4].copy_from_slice(&dx.to_le_bytes());
    bytes[4..8].copy_from_slice(&dy.to_le_bytes());
    bytes[8..12].copy_from_slice(&wheel.to_le_bytes());
    bytes[12..16].copy_from_slice(&buttons);
    MousePacket(bytes)
}

fn owner() -> StagedInput {
    // These are explicit test-fixture choices, NOT original runtime defaults.
    StagedInput::from_process_cells(
        RetainedCells {
            mouse_current: MousePacket([0; 16]),
            mouse_prior: MousePacket([0; 16]),
            keyboard_current: [0; 256],
            keyboard_prior: [0; 256],
            logical_pointer: LogicalPointer { x: 41, y: 79 },
            left_override: 0,
            right_override: 0,
            double_click: 0,
            keyboard_poll_tick: 0,
            device_event_count: 123,
            input_enabled: 1,
            focus_transition: 0,
            recording: 0,
            space_recorded: 0,
        },
        CharacterQueue::initialize([0xa5; 16]),
    )
}

struct FixtureDevice {
    mouse: [u8; 16],
    keyboard: [u8; 256],
    events: Vec<KeyEvent>,
    hresult: i32,
    calls: Vec<&'static str>,
}

impl FixtureDevice {
    fn explicit_zero_fixture() -> Self {
        Self {
            mouse: [0; 16],
            keyboard: [0; 256],
            events: vec![],
            hresult: 0,
            calls: vec![],
        }
    }
}

impl PortableDeviceAdapter for FixtureDevice {
    fn mouse_state(&mut self, out: &mut [u8; 16]) -> Result<i32, BoundaryError> {
        self.calls.push("poll0");
        *out = self.mouse;
        Ok(self.hresult)
    }
    fn keyboard_state(&mut self, out: &mut [u8; 256]) -> Result<i32, BoundaryError> {
        self.calls.push("poll1-unbuffered");
        *out = self.keyboard;
        Ok(self.hresult)
    }
    fn keyboard_events(&mut self, requested_count: u32) -> Result<BufferedKeyboard, BoundaryError> {
        assert_eq!(requested_count, 16);
        self.calls.push("poll1-buffered");
        BufferedKeyboard::admit(self.hresult, self.events.len() as u32, &self.events)
    }
}

fn selected_event(input: &mut StagedInput) -> Option<PointerEvent> {
    let ticket = input.prepare_selected_pointer_dispatch(false)?;
    let event = ticket.event();
    // This test records a selected delivery only. Actual slot14 routing is a
    // host integration test obligation; there is no production callback stub.
    let _ = input.finish_selected_pointer_dispatch(ticket, false);
    Some(event)
}

#[test]
fn pump_precedes_old_group_packet_then_poll0_admits_press_hold_release() {
    let mut input = owner();
    let mut device = FixtureDevice::explicit_zero_fixture();
    device.mouse = mouse(-7, 11, 0, [0x80, 0x80, 0x55, 0xaa]).0;
    input.window_input_effect(WindowInputEffect::PointerMoved(LogicalPointer {
        x: 620,
        y: 301,
    }));
    input.window_input_effect(WindowInputEffect::ButtonMessage { wparam: 1 });
    assert_eq!(
        input.cells().logical_pointer,
        LogicalPointer { x: 620, y: 301 }
    );
    assert_eq!(selected_event(&mut input), None); // groups still see old up/up
    input.poll0_live(&mut device).unwrap();
    assert_eq!(
        input.cells().mouse_current,
        mouse(-7, 11, 0, [1, 0, 0x55, 0xaa])
    );
    assert_eq!(input.cells().mouse_prior, MousePacket([0; 16]));
    assert_eq!(input.cells().device_event_count, 16);
    assert_eq!(selected_event(&mut input).unwrap().code, 4);
    input.poll0_live(&mut device).unwrap();
    assert_eq!(selected_event(&mut input).unwrap().code, 5);

    input.window_input_effect(WindowInputEffect::ButtonMessage { wparam: 0 });
    assert_eq!(selected_event(&mut input).unwrap().code, 5); // early OS release isn't poll release
    input.poll0_live(&mut device).unwrap();
    assert_eq!(selected_event(&mut input).unwrap().code, 6);
    input.poll0_live(&mut device).unwrap();
    assert_eq!(selected_event(&mut input), None);
}

#[test]
fn exact_nonzero_buttons_and_wheel_precedence() {
    let cases = [
        ([1, 0], [0, 0], 0, 0, Some(4)),
        ([0x80, 0], [2, 0], 100, 0, Some(5)),
        ([0, 0], [0x80, 0], -1, 0, Some(6)),
        ([0, 0], [0, 0], 1, 0, Some(8)),
        ([0, 0], [0, 0], -1, 0, Some(9)),
        ([0, 0], [0, 0], 1, -1, None),
        ([1, 2], [0, 0], 1, 0, None),
        ([0, 1], [0, 0], -1, 0, None),
    ];
    for (current, prior, wheel, prior_wheel, expected) in cases {
        let mut input = owner();
        input.cells.mouse_current = mouse(0, 0, wheel, [current[0], current[1], 0xff, 0xff]);
        input.cells.mouse_prior = mouse(0, 0, prior_wheel, [prior[0], prior[1], 0, 0]);
        assert_eq!(selected_event(&mut input).map(|e| e.code), expected);
    }
    let mut input = owner();
    input.cells.mouse_prior.0[13] = 2;
    assert!(input.right_fell());
    assert!(input.right_fell()); // predicate, not a destructive edge/tap take
}

#[test]
fn keyboard_copies_all256_before_exact_native_index_updates() {
    let mut input = owner();
    let mut device = FixtureDevice::explicit_zero_fixture();
    input.cells.keyboard_current[0x11] = 0x80;
    input.cells.keyboard_current[0xff] = 0x81;
    device.events = vec![
        KeyEvent {
            native_index: 0x11,
            data: 0x7f,
        }, // release, not bool down
        KeyEvent {
            native_index: 0xff,
            data: 0x80,
        },
        KeyEvent {
            native_index: 0x38,
            data: 0x180,
        },
        KeyEvent {
            native_index: 0x38,
            data: 0x100,
        }, // ordered second event wins
    ];
    let report = input
        .poll1_live(KeyboardMode::Buffered16, &mut device)
        .unwrap();
    assert_eq!(report.event_count, Some(4));
    assert_eq!(input.cells().device_event_count, 4);
    assert_eq!(input.cells().keyboard_prior[0x11], 0x80);
    assert_eq!(input.cells().keyboard_prior[0xff], 0x81);
    assert_eq!(input.cells().keyboard_current[0x11], 0);
    assert_eq!(input.cells().keyboard_current[0xff], 0x80);
    assert_eq!(input.cells().keyboard_current[0x38], 0);
    assert_eq!(input.cells().keyboard_poll_tick, 1);
    assert_eq!(device.calls, ["poll1-buffered"]);

    let before = input.cells().keyboard_current;
    for (index, byte) in device.keyboard.iter_mut().enumerate() {
        *byte = index as u8;
    }
    input
        .poll1_live(KeyboardMode::Unbuffered256, &mut device)
        .unwrap();
    assert_eq!(input.cells().keyboard_prior, before);
    assert_eq!(input.cells().keyboard_current, device.keyboard); // no 0x80 normalization here
    assert_eq!(input.cells().keyboard_current[0x7f], 0x7f);
    assert_eq!(input.cells().keyboard_current[0xff], 0xff);
    assert_eq!(input.cells().device_event_count, 16);
    assert_eq!(
        KeyboardMode::from_receiver_buffering_word(0),
        KeyboardMode::Unbuffered256
    );
    assert_eq!(
        KeyboardMode::from_receiver_buffering_word(2),
        KeyboardMode::Buffered16
    );
}

#[test]
fn buffered_limit_and_indices_fail_explicitly_instead_of_remapping() {
    let records = [KeyEvent {
        native_index: 255,
        data: 0x80,
    }; 16];
    assert!(BufferedKeyboard::admit(0, 16, &records).is_ok());
    let seventeen = [KeyEvent {
        native_index: 1,
        data: 0x80,
    }; 17];
    assert_eq!(
        BufferedKeyboard::admit(0, 17, &seventeen),
        Err(BoundaryError::BufferedCountExceeds16(17))
    );
    assert_eq!(
        BufferedKeyboard::admit(
            0,
            1,
            &[KeyEvent {
                native_index: 256,
                data: 0x80
            }]
        ),
        Err(BoundaryError::KeyboardIndexOutside256(256))
    );
}

#[test]
fn keyboard_press_held_release_prior_is_poll_staged_not_char_or_tail_staged() {
    let mut input = owner();
    let mut device = FixtureDevice::explicit_zero_fixture();
    for (down, expected_current, expected_prior) in [
        (true, 0x80, 0),
        (true, 0x80, 0x80),
        (false, 0, 0x80),
        (false, 0, 0),
    ] {
        let before = input.cells().keyboard_current[0xc8];
        input.window_input_effect(WindowInputEffect::Character {
            wparam: b'W'.into(),
        });
        assert_eq!(input.cells().keyboard_current[0xc8], before); // CHAR isn't a held-key producer
        device.events = vec![KeyEvent {
            native_index: 0xc8,
            data: if down { 0x80 } else { 0 },
        }];
        input
            .poll1_live(KeyboardMode::Buffered16, &mut device)
            .unwrap();
        assert_eq!(input.cells().keyboard_current[0xc8], expected_current);
        assert_eq!(input.cells().keyboard_prior[0xc8], expected_prior);
        // Merely reading a camera/group input packet doesn't consume a key.
        assert_eq!(input.cells().keyboard_current[0xc8], expected_current);
    }
}

#[test]
fn hresult_failure_preserves_actual_writes_and_native_copy_order() {
    let mut input = owner();
    input.cells.mouse_current = mouse(21, -13, 7, [9, 8, 7, 6]);
    input.cells.left_override = 1;
    input.cells.right_override = 2;
    let old_mouse = input.cells().mouse_current;
    let mut device = FixtureDevice::explicit_zero_fixture();
    device.hresult = -1;
    device.mouse = mouse(3, 4, -2, [0xff; 4]).0;
    assert_eq!(input.poll0_live(&mut device).unwrap().hresult, -1);
    assert_eq!(input.cells().mouse_prior, old_mouse);
    assert_eq!(
        input.cells().mouse_current,
        mouse(3, 4, -2, [1, 2, 0xff, 0xff])
    );
    input.cells.keyboard_current[10] = 0x80;
    device.events = vec![KeyEvent {
        native_index: 10,
        data: 0,
    }];
    input.cells.keyboard_poll_tick = u32::MAX;
    assert_eq!(
        input
            .poll1_live(KeyboardMode::Buffered16, &mut device)
            .unwrap()
            .hresult,
        -1
    );
    assert_eq!(input.cells().keyboard_prior[10], 0x80);
    assert_eq!(input.cells().keyboard_current[10], 0x80); // failed event list is not applied
    assert_eq!(input.cells().keyboard_poll_tick, 0); // native unsigned wrap
    device.keyboard[20] = 0x7f;
    input
        .poll1_live(KeyboardMode::Unbuffered256, &mut device)
        .unwrap();
    assert_eq!(input.cells().keyboard_current[20], 0x7f); // failure doesn't clear device writes
}

#[test]
fn no_owned_device_reports_boundary_with_prior_already_copied() {
    struct Unowned;
    impl PortableDeviceAdapter for Unowned {
        fn mouse_state(&mut self, _: &mut [u8; 16]) -> Result<i32, BoundaryError> {
            Err(BoundaryError::AdapterUnavailable("mouse"))
        }
        fn keyboard_state(&mut self, _: &mut [u8; 256]) -> Result<i32, BoundaryError> {
            Err(BoundaryError::AdapterUnavailable("keyboard"))
        }
        fn keyboard_events(
            &mut self,
            requested_count: u32,
        ) -> Result<BufferedKeyboard, BoundaryError> {
            assert_eq!(requested_count, 16);
            Err(BoundaryError::AdapterUnavailable("buffered keyboard"))
        }
    }
    let mut input = owner();
    input.cells.mouse_current.0[14] = 0x55;
    input.cells.keyboard_current[255] = 0x80;
    let before = input.cells().clone();
    assert_eq!(
        input.poll0_live(&mut Unowned),
        Err(BoundaryError::AdapterUnavailable("mouse"))
    );
    assert_eq!(input.cells().mouse_prior, before.mouse_current);
    assert_eq!(input.cells().mouse_current, before.mouse_current);
    assert!(
        input
            .poll1_live(KeyboardMode::Unbuffered256, &mut Unowned)
            .is_err()
    );
    assert_eq!(input.cells().keyboard_prior, before.keyboard_current);
    assert_eq!(input.cells().keyboard_poll_tick, before.keyboard_poll_tick); // incomplete call, not success
}

#[test]
fn char_storage_overwrites_slot0_and_event10_takes_twice() {
    let mut input = owner();
    for byte in [b'A', b'B', b'C'] {
        input.window_input_effect(WindowInputEffect::Character {
            wparam: byte.into(),
        });
    }
    assert_eq!(&input.characters().slots()[1..], &[0xa5; 15]);
    assert_eq!(input.characters().controls().count, 3);
    let ticket = input.prepare_selected_pointer_dispatch(false).unwrap();
    assert_eq!(
        ticket.event(),
        PointerEvent {
            code: 10,
            value: b'C'
        }
    );
    assert_eq!(input.characters().controls().count, 2);
    assert_eq!(
        input.finish_selected_pointer_dispatch(ticket, false),
        Some(b'C')
    );
    assert_eq!(input.characters().controls().count, 1);
    assert_eq!(input.take_character(), b'C');
    assert_eq!(input.take_character(), 0);
}

#[test]
fn char_post_dispatch_reloads_key_target_and_character_state() {
    let mut input = owner();
    input.window_input_effect(WindowInputEffect::Character { wparam: 0x141 }); // low byte only
    input.cells.mouse_current.0[12] = 1;
    let press = input.prepare_selected_pointer_dispatch(false).unwrap();
    assert_eq!(press.event().code, 4);
    assert_eq!(input.characters().controls().count, 1); // pointer takes none before dispatch
    // Real callback can install a key target, preventing extra consumption.
    assert_eq!(input.finish_selected_pointer_dispatch(press, true), None);
    assert_eq!(input.take_character(), b'A');

    input.window_input_effect(WindowInputEffect::Character {
        wparam: b'B'.into(),
    });
    let press = input.prepare_selected_pointer_dispatch(true).unwrap();
    input.clear_characters_4e54d0(); // callback clears and appends
    input.window_input_effect(WindowInputEffect::Character {
        wparam: b'Z'.into(),
    });
    assert_eq!(
        input.finish_selected_pointer_dispatch(press, false),
        Some(b'Z')
    );
    assert_eq!(input.character_available_byte(), 0);
}

#[test]
fn chars_wait_without_selected_delivery_or_with_key_target_or_right_hold() {
    let mut input = owner();
    input.window_input_effect(WindowInputEffect::Character {
        wparam: b'X'.into(),
    });
    // No selected recipient: the host must not call prepare at all.
    assert_eq!(input.characters().controls().count, 1);
    assert!(input.prepare_selected_pointer_dispatch(true).is_none());
    assert!(!input.key_target_char_admitted(2)); // exact byte equality, not generic nonzero
    assert!(input.key_target_char_admitted(1));
    assert_eq!(input.characters().controls().count, 1); // key-target predicate doesn't take
    input.cells.mouse_current.0[13] = 2;
    assert!(input.prepare_selected_pointer_dispatch(false).is_none());
    assert_eq!(input.characters().controls().count, 1);
}

#[test]
fn char_signed_count_and_wrapping_are_not_fifo_capacity_rules() {
    let mut queue = CharacterQueue::initialize([0x55; 16]);
    for _ in 0..129 {
        queue.append(b'K');
    }
    assert_eq!(queue.take(), b'K');
    assert_eq!(queue.controls().count, 128);
    assert_eq!(queue.available_byte(), 0); // signed count -128 clears availability
    queue.clear();
    for _ in 0..256 {
        queue.append(b'Q');
    }
    assert_eq!(queue.controls().count, 0);
    assert_eq!(queue.available_byte(), 1);
    assert_eq!(queue.take(), b'Q');
    assert_eq!(queue.controls().count, 255);
    assert_eq!(queue.available_byte(), 0);
    assert_eq!(queue.slots()[15], 0x55); // never silently wipe untouched backing
}

#[test]
fn char_space_recording_and_doubleclick_are_message_owned_not_tail_owned() {
    let mut input = owner();
    input.cells.recording = 1;
    input.window_input_effect(WindowInputEffect::Character { wparam: 0x120 });
    assert_eq!(input.cells().space_recorded, 1);
    input.cells.recording = 0; // subsequent fixture polls are live/nonrecording
    input.window_input_effect(WindowInputEffect::LeftDoubleClick);
    assert_eq!(input.cells().double_click, 1);
    let mut device = FixtureDevice::explicit_zero_fixture();
    for _ in 0..3 {
        input.poll0_live(&mut device).unwrap();
        input
            .poll1_live(KeyboardMode::Unbuffered256, &mut device)
            .unwrap();
        assert_eq!(input.cells().double_click, 1); // no invented central consume
    }
    input.window_input_effect(WindowInputEffect::Other); // paint/key/etc also clears
    assert_eq!(input.cells().double_click, 0);
    input.window_input_effect(WindowInputEffect::LeftDoubleClick);
    input.window_input_effect(WindowInputEffect::PointerMoved(LogicalPointer {
        x: 4,
        y: 5,
    }));
    assert_eq!(input.cells().double_click, 0); // last actual message controls marker
}

#[test]
fn focus_input_slice_retains_held_packets_queue_overrides_and_pointer() {
    let mut input = owner();
    input.cells.mouse_current.0[12] = 1;
    input.cells.mouse_prior.0[12] = 1;
    input.cells.keyboard_current[0x38] = 0x80;
    input.cells.keyboard_prior[0x38] = 0x80;
    input.window_input_effect(WindowInputEffect::ButtonMessage { wparam: 3 });
    input.window_input_effect(WindowInputEffect::Character {
        wparam: b'H'.into(),
    });
    let before = input.clone();
    input.focus_loss_cells_after_device_calls();
    let mut expected = before.clone();
    expected.cells.input_enabled = 0;
    assert_eq!(input, expected);
    // Focus gates later poll/controller branch, not the group's retained bytes.
    input.focus_gain_cells_after_device_calls();
    expected.cells.input_enabled = 1;
    expected.cells.focus_transition = 1;
    assert_eq!(input, expected);
    // Menu/load/refresh and pause have no methods which reset this process owner.
    input.window_closing_transition_cell();
    expected.cells.focus_transition = 0;
    assert_eq!(input, expected);
    let registrations = [(4u32, true), (1, true), (1, true), (10, true)];
    let enabled_after_mask2: Vec<bool> = registrations
        .iter()
        .map(|(mask, enabled)| *enabled && *mask & 2 == 0)
        .collect();
    assert_eq!(enabled_after_mask2, [true, true, true, false]);
    // Actual manager pause counters/reloads and frontend calls remain host integration tests.
}

#[test]
fn three_central_catchup_and_speed_repeat_old_packets_not_device_admission() {
    for primary_repeat in [1, 4] {
        let mut input = owner();
        let mut device = FixtureDevice::explicit_zero_fixture();
        input.window_input_effect(WindowInputEffect::ButtonMessage { wparam: 1 });
        let mut groups = vec![];
        let mut journal = vec!["message-pump"];
        let mut tails = 0;
        let mut camera_dx = vec![];
        for central in 0..3 {
            for _ in 0..primary_repeat {
                journal.push("primary");
                groups.push((
                    central,
                    "primary",
                    selected_event(&mut input).map(|e| e.code),
                ));
            }
            for group in ["secondary", "tertiary"] {
                journal.push(group);
                groups.push((central, group, selected_event(&mut input).map(|e| e.code)));
            }
            device.mouse = mouse(if central == 0 { 17 } else { 0 }, 0, 0, [0; 4]).0;
            journal.push("poll0");
            input.poll0_live(&mut device).unwrap();
            journal.push("poll1");
            input
                .poll1_live(KeyboardMode::Buffered16, &mut device)
                .unwrap();
            journal.push("selection"); // marker only, no successful selection callback
            journal.push("tail");
            tails += 1;
            camera_dx.push(input.cells().mouse_current.dx());
        }
        assert_eq!(tails, 3);
        assert_eq!(
            device.calls,
            [
                "poll0",
                "poll1-buffered",
                "poll0",
                "poll1-buffered",
                "poll0",
                "poll1-buffered"
            ]
        );
        assert_eq!(input.cells().keyboard_poll_tick, 3);
        assert_eq!(camera_dx, [17, 0, 0]); // producer drains relative movement at polls, not tail
        assert_eq!(groups.len(), 3 * (primary_repeat + 2));
        for (central, _, event) in groups {
            assert_eq!(
                event,
                match central {
                    0 => None,
                    1 => Some(4),
                    _ => Some(5),
                }
            );
        }
        assert_eq!(journal.iter().filter(|entry| **entry == "poll0").count(), 3);
        assert_eq!(journal.iter().filter(|entry| **entry == "tail").count(), 3);
        for pass in journal[1..].chunks(primary_repeat + 6) {
            assert!(
                pass[..primary_repeat]
                    .iter()
                    .all(|entry| *entry == "primary")
            );
            assert_eq!(
                &pass[primary_repeat..],
                &[
                    "secondary",
                    "tertiary",
                    "poll0",
                    "poll1",
                    "selection",
                    "tail"
                ]
            );
        }
        // Original primary repeats DO repeat press4 on the same retained packet.
        // Deduplicating these into one tap would be another fidelity defect.
    }
}

#[test]
fn failed_buffered_poll_retains_returned_count_without_reading_invalid_records() {
    struct Failed;
    impl PortableDeviceAdapter for Failed {
        fn mouse_state(&mut self, _: &mut [u8; 16]) -> Result<i32, BoundaryError> {
            Err(BoundaryError::AdapterUnavailable("unused"))
        }
        fn keyboard_state(&mut self, _: &mut [u8; 256]) -> Result<i32, BoundaryError> {
            Err(BoundaryError::AdapterUnavailable("unused"))
        }
        fn keyboard_events(&mut self, count: u32) -> Result<BufferedKeyboard, BoundaryError> {
            assert_eq!(count, 16);
            BufferedKeyboard::admit(-7, 16, &[])
        }
    }
    let mut input = owner();
    input.cells.keyboard_current = [0x5a; 256];
    input.cells.keyboard_poll_tick = u32::MAX;
    let report = input
        .poll1_live(KeyboardMode::Buffered16, &mut Failed)
        .unwrap();
    assert_eq!(
        report,
        PollReport {
            hresult: -7,
            event_count: Some(16)
        }
    );
    assert_eq!(input.cells.keyboard_prior, [0x5a; 256]);
    assert_eq!(input.cells.keyboard_current, [0x5a; 256]);
    assert_eq!(input.cells.device_event_count, 16);
    assert_eq!(input.cells.keyboard_poll_tick, 0);
    assert!(
        BufferedKeyboard::admit(
            -7,
            16,
            &[KeyEvent {
                native_index: u32::MAX,
                data: 0
            }]
        )
        .is_ok()
    );
    assert_eq!(
        BufferedKeyboard::admit(0, 1, &[]),
        Err(BoundaryError::BufferedRecordsMissing {
            returned_count: 1,
            available: 0
        })
    );
    let zero = BufferedKeyboard::admit(
        0,
        0,
        &[KeyEvent {
            native_index: u32::MAX,
            data: 0,
        }],
    )
    .unwrap();
    assert_eq!(zero.returned_count, 0);
    assert!(zero.records.is_empty());
}
struct PartialDevice {
    unowned: bool,
}
impl PortableDeviceAdapter for PartialDevice {
    fn mouse_state(&mut self, out: &mut [u8; 16]) -> Result<i32, BoundaryError> {
        out[0] = 0xaa;
        if self.unowned {
            Err(BoundaryError::AdapterUnavailable("partial mouse"))
        } else {
            Ok(-3)
        }
    }
    fn keyboard_state(&mut self, out: &mut [u8; 256]) -> Result<i32, BoundaryError> {
        out[255] = 0x80;
        if self.unowned {
            Err(BoundaryError::AdapterUnavailable("partial keyboard"))
        } else {
            Ok(-4)
        }
    }
    fn keyboard_events(&mut self, _: u32) -> Result<BufferedKeyboard, BoundaryError> {
        Err(BoundaryError::AdapterUnavailable("unused"))
    }
}
#[test]
fn negative_hresult_retains_true_partial_destination_writes_and_untouched_bytes() {
    let mut input = owner();
    input.cells.mouse_current = MousePacket([0x5a; 16]);
    input.cells.left_override = 1;
    input.cells.right_override = 2;
    let mut device = PartialDevice { unowned: false };
    assert_eq!(input.poll0_live(&mut device).unwrap().hresult, -3);
    assert_eq!(input.cells.mouse_prior, MousePacket([0x5a; 16]));
    let mut expected = [0x5a; 16];
    expected[0] = 0xaa;
    expected[12] = 1;
    expected[13] = 2;
    assert_eq!(input.cells.mouse_current, MousePacket(expected));
    input.cells.keyboard_current = [0x5a; 256];
    input.cells.keyboard_poll_tick = 9;
    assert_eq!(
        input
            .poll1_live(KeyboardMode::Unbuffered256, &mut device)
            .unwrap()
            .hresult,
        -4
    );
    assert_eq!(input.cells.keyboard_prior, [0x5a; 256]);
    let mut expected = [0x5a; 256];
    expected[255] = 0x80;
    assert_eq!(input.cells.keyboard_current, expected);
    assert_eq!(input.cells.keyboard_poll_tick, 10);
}
#[test]
fn write_then_ownership_error_leaves_only_incomplete_poll_prefix_and_device_writes() {
    let mut input = owner();
    input.cells.mouse_current = MousePacket([0x5a; 16]);
    input.cells.left_override = 1;
    input.cells.right_override = 2;
    let mut device = PartialDevice { unowned: true };
    assert!(input.poll0_live(&mut device).is_err());
    assert_eq!(input.cells.mouse_prior, MousePacket([0x5a; 16]));
    let mut expected = [0x5a; 16];
    expected[0] = 0xaa;
    assert_eq!(input.cells.mouse_current, MousePacket(expected));
    assert_eq!(input.cells.device_event_count, 16);
    input.cells.keyboard_current = [0x5a; 256];
    input.cells.keyboard_poll_tick = 9;
    assert!(
        input
            .poll1_live(KeyboardMode::Unbuffered256, &mut device)
            .is_err()
    );
    assert_eq!(input.cells.keyboard_prior, [0x5a; 256]);
    let mut expected = [0x5a; 256];
    expected[255] = 0x80;
    assert_eq!(input.cells.keyboard_current, expected);
    assert_eq!(input.cells.keyboard_poll_tick, 9);
    assert_eq!(input.cells.device_event_count, 16);
}

#[test]
fn split_keyboard_poll_retains_device_prefix_until_late_recording_finishes() {
    let mut input = owner();
    input.cells.keyboard_current = [0x5a; 256];
    input.cells.keyboard_poll_tick = u32::MAX;
    let mut device = PartialDevice { unowned: false };
    let pending = input
        .poll1_live_before_recording(KeyboardMode::Unbuffered256, &mut device)
        .unwrap();
    assert_eq!(pending.report().hresult, -4);
    assert_eq!(input.cells.keyboard_prior, [0x5a; 256]);
    let mut expected = [0x5a; 256];
    expected[255] = 0x80;
    assert_eq!(input.cells.keyboard_current, expected);
    assert_eq!(input.cells.keyboard_poll_tick, u32::MAX);
    // A real caller can observe a recording transition after device delivery.
    // This synthetic write does not supply an actual recording-stream provider.
    input.cells.recording = 1;
    input.window_input_effect(WindowInputEffect::Character { wparam: 0x20 });
    assert_eq!(input.cells.space_recorded, 1);
    assert_eq!(input.cells.keyboard_poll_tick, u32::MAX);
    let report = input.finish_poll1_after_recording(pending);
    assert_eq!(report.hresult, -4);
    assert_eq!(input.cells.keyboard_poll_tick, 0);
    assert_eq!(input.take_character(), 0x20);
}

#[test]
fn unresolved_recording_keeps_prefix_without_advancing_keyboard_tick() {
    let mut input = owner();
    input.cells.keyboard_current = [0x5a; 256];
    input.cells.keyboard_poll_tick = 9;
    let mut device = PartialDevice { unowned: false };
    let pending = input
        .poll1_live_before_recording(KeyboardMode::Unbuffered256, &mut device)
        .unwrap();
    // Dropping unresolved work represents a rejected/missing recording handler.
    // Prior and device writes remain committed; successful completion is absent.
    drop(pending);
    assert_eq!(input.cells.keyboard_poll_tick, 9);
    assert_eq!(input.cells.keyboard_prior, [0x5a; 256]);
    assert_eq!(input.cells.keyboard_current[255], 0x80);
    assert_eq!(input.cells.keyboard_current[254], 0x5a);
    device.unowned = true;
    assert!(
        input
            .poll1_live_before_recording(KeyboardMode::Unbuffered256, &mut device)
            .is_err()
    );
    assert_eq!(input.cells.keyboard_poll_tick, 9);
}

#[test]
fn live_wrapper_preserves_buffered_and_unbuffered_poll_results_and_cells() {
    for mode in [KeyboardMode::Unbuffered256, KeyboardMode::Buffered16] {
        for hresult in [0, -4] {
            let mut complete = owner();
            complete.cells.keyboard_current = [0x5a; 256];
            complete.cells.keyboard_poll_tick = u32::MAX;
            let mut split = complete.clone();
            let fixture = || FixtureDevice {
                mouse: [0; 16],
                keyboard: [0x80; 256],
                events: vec![KeyEvent {
                    native_index: 255,
                    data: 0x80,
                }],
                hresult,
                calls: vec![],
            };
            let mut complete_device = fixture();
            let mut split_device = fixture();
            let complete_report = complete.poll1_live(mode, &mut complete_device).unwrap();
            let pending = split
                .poll1_live_before_recording(mode, &mut split_device)
                .unwrap();
            assert_eq!(split.cells.keyboard_poll_tick, u32::MAX);
            let split_report = split.finish_poll1_after_recording(pending);
            assert_eq!(complete_report, split_report);
            assert_eq!(complete, split);
            assert_eq!(complete_device.calls, split_device.calls);
        }
    }
}
