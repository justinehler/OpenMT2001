//! Retained native mouse, keyboard and character input for live, non-replay polls.
//! The host dispatches groups against the old packet before polling mouse then
//! keyboard. Device acquisition, GUI routing and camera delivery belong to the host.

pub const MOUSE_BYTES: usize = 16;
pub const KEYBOARD_BYTES: usize = 256;
pub const BUFFERED_EVENT_LIMIT: usize = 16;
pub const LEFT_BYTE: usize = 12;
pub const RIGHT_BYTE: usize = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MousePacket(pub [u8; MOUSE_BYTES]);

impl MousePacket {
    pub fn dx(self) -> i32 {
        i32::from_le_bytes(self.0[0..4].try_into().unwrap())
    }
    pub fn dy(self) -> i32 {
        i32::from_le_bytes(self.0[4..8].try_into().unwrap())
    }
    pub fn wheel(self) -> i32 {
        i32::from_le_bytes(self.0[8..12].try_into().unwrap())
    }
    pub fn left(self) -> u8 {
        self.0[LEFT_BYTE]
    }
    pub fn right(self) -> u8 {
        self.0[RIGHT_BYTE]
    }
}

/// Already resolved native cells. No pixel scaling or native numeric equivalence
/// is inferred here. 4e4f50's provider must supply the actual two output words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalPointer {
    pub x: i32, // 93c5c0
    pub y: i32, // 93c5dc
}

/// Explicit process seed, not Default/calloc. Do not reseed on menu/load/refresh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedCells {
    pub mouse_current: MousePacket,             // 93c600..60f
    pub mouse_prior: MousePacket,               // 93c5c8..5d7
    pub keyboard_current: [u8; KEYBOARD_BYTES], // 93c710..80f
    pub keyboard_prior: [u8; KEYBOARD_BYTES],   // 93c610..70f
    pub logical_pointer: LogicalPointer,
    pub left_override: u32,      // 93c5f0, full word from 4e4fd0
    pub right_override: u32,     // 93c5f4
    pub double_click: u8,        // 93c5e0, this-WndProc marker
    pub keyboard_poll_tick: u32, // 93c5ec
    pub device_event_count: u32, // receiver+44; poll entry writes 16 even for mouse
    pub input_enabled: u8,       // 8b2374, not a group/tail gate
    pub focus_transition: u8,    // 8b22dd
    pub recording: u8,           // receiver+19218; queue's space side effect only
    pub space_recorded: u8,      // 93c811
}

/// Exact reachable constructor-initialized CHAR storage, 4e5490..4e5517.
/// The native code does NOT increment write/read indices. Appends overwrite
/// slot 0; the wrapping count can still require several takes of that same byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterQueue {
    slots: [u8; 16], // receiver+21..30, initialization leaves these untouched
    available: u8,   // +20
    write_index: u8, // +31
    read_index: u8,  // +32
    count: u8,       // +33
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterControls {
    pub available: u8,
    pub write_index: u8,
    pub read_index: u8,
    pub count: u8,
}

impl CharacterQueue {
    /// Apply only the four constructor/clear writes proven by 4e4900/4e54d0.
    /// Caller supplies untouched backing cells; no zero-filled heap assumption.
    pub fn initialize(untouched_slots: [u8; 16]) -> Self {
        Self {
            slots: untouched_slots,
            available: 0,
            write_index: 0,
            read_index: 0,
            count: 0,
        }
    }

    pub fn controls(&self) -> CharacterControls {
        CharacterControls {
            available: self.available,
            write_index: self.write_index,
            read_index: self.read_index,
            count: self.count,
        }
    }

    pub fn slots(&self) -> &[u8; 16] {
        &self.slots
    }
    pub fn available_byte(&self) -> u8 {
        self.available
    }

    fn append(&mut self, value: u8) {
        // Indices start at zero and no admitted method increments them. Native
        // arbitrary/aliased receiver memory is deliberately not an import API.
        self.available = 1;
        self.slots[self.write_index as usize] = value;
        self.count = self.count.wrapping_add(1);
        if self.write_index == 16 {
            self.write_index = 0;
        }
    }

    pub fn clear(&mut self) {
        self.read_index = 0;
        self.write_index = 0;
        self.count = 0;
        self.available = 0;
    }

    pub fn take(&mut self) -> u8 {
        if self.available == 0 {
            return 0;
        }
        self.count = self.count.wrapping_sub(1);
        let value = self.slots[self.read_index as usize];
        self.read_index = 0;
        if (self.count as i8) <= 0 {
            self.available = 0;
        }
        value
    }
}

/// One actual WndProc invocation's finite input effects. Other MUST also be
/// delivered: non-input messages clear the double-click marker too (4c6977).
/// Host text -> original byte encoding and pointer scaling are adapter seams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowInputEffect {
    PointerMoved(LogicalPointer), // WM_MOUSEMOVE, after admitted 4e4f50 conversion
    ButtonMessage { wparam: u32 }, // only WM201/202/204/205
    Character { wparam: u32 },    // WM102 -> low byte, not a key scan index
    LeftDoubleClick,              // WM203, does not itself change overrides
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub native_index: u32, // DIDEVICEOBJECTDATA.dwOfs, exact 0..255
    pub data: u32,         // dwData; only low byte & 0x80 is stored
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundaryError {
    AdapterUnavailable(&'static str),
    BufferedCountExceeds16(usize),
    KeyboardIndexOutside256(u32),
    BufferedRecordsMissing {
        returned_count: usize,
        available: usize,
    },
}

/// Validated portable admission. No out-of-bounds native memory behavior is
/// silently emulated; overflow/replay blobs need a separately reviewed owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferedKeyboard {
    hresult: i32,
    returned_count: u32,
    records: Vec<KeyEvent>,
}

impl BufferedKeyboard {
    /// The device receives input count16. Its returned count is independent
    /// of readable records, especially after a native negative HRESULT.
    pub fn admit(
        hresult: i32,
        returned_count: u32,
        records: &[KeyEvent],
    ) -> Result<Self, BoundaryError> {
        let count = returned_count as usize;
        if count > BUFFERED_EVENT_LIMIT {
            return Err(BoundaryError::BufferedCountExceeds16(count));
        }
        if hresult < 0 {
            return Ok(Self {
                hresult,
                returned_count,
                records: Vec::new(),
            });
        }
        let used = records
            .get(..count)
            .ok_or(BoundaryError::BufferedRecordsMissing {
                returned_count: count,
                available: records.len(),
            })?;
        for record in used {
            if record.native_index >= KEYBOARD_BYTES as u32 {
                return Err(BoundaryError::KeyboardIndexOutside256(record.native_index));
            }
        }
        Ok(Self {
            hresult,
            returned_count,
            records: used.to_vec(),
        })
    }
}

/// Required live-device calls, no successful default implementations. The
/// destination is the real owner buffer: a failing device may partially write
/// it. Return the actual HRESULT; do not invent zero device state on error.
/// Err means no portable ownership, not native HRESULT handling or success.
pub trait PortableDeviceAdapter {
    fn mouse_state(&mut self, destination: &mut [u8; 16]) -> Result<i32, BoundaryError>;
    fn keyboard_state(&mut self, destination: &mut [u8; 256]) -> Result<i32, BoundaryError>;
    fn keyboard_events(&mut self, requested_count: u32) -> Result<BufferedKeyboard, BoundaryError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardMode {
    Unbuffered256,
    Buffered16,
}

impl KeyboardMode {
    pub fn from_receiver_buffering_word(word: u32) -> Self {
        if word == 0 {
            Self::Unbuffered256
        } else {
            Self::Buffered16
        }
    }
}

/// The live keyboard byte/event work completed, but native4e53c9 recording
/// and4e53e2 tick have NOT. Linear ticket for the SAME StagedInput owner.
/// Missing recording providers must retain prior/device writes and NOT tick.
#[derive(Debug)]
#[must_use = "Resolve actual recording gate/provider, then finish on SAME StagedInput"]
pub struct KeyboardPollBeforeRecording { report: PollReport }
impl KeyboardPollBeforeRecording {
    pub fn report(&self) -> PollReport { self.report }
}

/// HRESULT remains observable, including native failures; the native 4e48f0
/// reporter returns without clearing state. This does not assert hardware OK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PollReport {
    pub hresult: i32,
    pub event_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerEvent {
    pub code: u32,
    pub value: u8,
}

/// Non-Clone/Copy ticket for the post-dispatch CHAR take. The host must actually
/// dispatch through the selected node's slot14 between prepare and finish.
#[derive(Debug)]
#[must_use = "Dispatch through actual slot14, then finish using live key-target state"]
pub struct PreparedPointerDispatch {
    event: PointerEvent,
}

impl PreparedPointerDispatch {
    pub fn event(&self) -> PointerEvent {
        self.event
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedInput {
    cells: RetainedCells,
    characters: CharacterQueue,
}

impl StagedInput {
    pub fn from_process_cells(cells: RetainedCells, characters: CharacterQueue) -> Self {
        Self { cells, characters }
    }

    /// Read retained live state at each actual group/camera/provider use; do not
    /// retain a snapshot across a callback which can change it.
    pub fn cells(&self) -> &RetainedCells {
        &self.cells
    }
    pub fn characters(&self) -> &CharacterQueue {
        &self.characters
    }
    pub fn character_available_byte(&self) -> u8 {
        self.characters.available_byte()
    }
    pub fn take_character(&mut self) -> u8 {
        self.characters.take()
    }
    pub fn clear_characters_4e54d0(&mut self) {
        self.characters.clear();
    }

    /// Called once per actual host message, before manager dispatch, not once
    /// per callback or camera-tail. This owns input effects only, not WndProc's
    /// UI/capture/focus actions. Do not synthesize Other at each central.
    pub fn window_input_effect(&mut self, effect: WindowInputEffect) {
        self.cells.double_click = 0;
        match effect {
            WindowInputEffect::PointerMoved(position) => self.cells.logical_pointer = position,
            WindowInputEffect::ButtonMessage { wparam } => {
                self.cells.left_override = wparam & 1;
                self.cells.right_override = wparam & 2;
            }
            WindowInputEffect::Character { wparam } => {
                let value = wparam as u8;
                self.characters.append(value);
                if self.cells.recording != 0 && value == 0x20 {
                    self.cells.space_recorded = 1;
                }
            }
            WindowInputEffect::LeftDoubleClick => self.cells.double_click = 1,
            WindowInputEffect::Other => (),
        }
    }

    /// 4e51c0(mode0), live non-replay branch. Central's old-packet groups must
    /// already have run. Prior is copied even if the adapter later fails.
    /// Actual recording/replay stream providers remain outside this module.
    pub fn poll0_live(
        &mut self,
        adapter: &mut impl PortableDeviceAdapter,
    ) -> Result<PollReport, BoundaryError> {
        self.cells.device_event_count = 16;
        self.cells.mouse_prior = self.cells.mouse_current;
        let hresult = adapter.mouse_state(&mut self.cells.mouse_current.0)?;
        self.cells.mouse_current.0[LEFT_BYTE] = self.cells.left_override as u8;
        self.cells.mouse_current.0[RIGHT_BYTE] = self.cells.right_override as u8;
        Ok(PollReport {
            hresult,
            event_count: None,
        })
    }

    /// 4e51c0(mode1), live non-replay branch. Mode must be captured from the
    /// actual receiver+1c before entry, before copying the keyboard cells.
    pub fn poll1_live(
        &mut self,
        mode: KeyboardMode,
        adapter: &mut impl PortableDeviceAdapter,
    ) -> Result<PollReport, BoundaryError> {
        let pending = self.poll1_live_before_recording(mode, adapter)?;
        Ok(self.finish_poll1_after_recording(pending))
    }

    /// Same live poll implementation, stopped BEFORE recording gate/callback
    /// and tick. A source caller must observe receiver+19218 after device work;
    /// it cannot silently skip serialization if that callback enabled recording.
    pub fn poll1_live_before_recording(
        &mut self,
        mode: KeyboardMode,
        adapter: &mut impl PortableDeviceAdapter,
    ) -> Result<KeyboardPollBeforeRecording, BoundaryError> {
        self.cells.device_event_count = 16;
        self.cells.keyboard_prior = self.cells.keyboard_current;
        let report = match mode {
            KeyboardMode::Unbuffered256 => {
                let hresult = adapter.keyboard_state(&mut self.cells.keyboard_current)?;
                PollReport {
                    hresult,
                    event_count: None,
                }
            }
            KeyboardMode::Buffered16 => {
                let batch = adapter.keyboard_events(16)?;
                self.cells.device_event_count = batch.returned_count;
                if batch.hresult >= 0 {
                    for record in &batch.records {
                        self.cells.keyboard_current[record.native_index as usize] =
                            (record.data as u8) & 0x80;
                    }
                }
                PollReport {
                    hresult: batch.hresult,
                    event_count: Some(batch.returned_count as usize),
                }
            }
        };
        Ok(KeyboardPollBeforeRecording { report })
    }

    /// 4e53e2..ea, only after actual recording handling/admitted off gate.
    /// Consume once on SAME owner; never use a ticket to replay prior copies.
    pub fn finish_poll1_after_recording(&mut self, pending: KeyboardPollBeforeRecording) -> PollReport {
        self.cells.keyboard_poll_tick = self.cells.keyboard_poll_tick.wrapping_add(1);
        pending.report
    }

    pub fn right_fell(&self) -> bool {
        self.cells.mouse_prior.right() != 0 && self.cells.mouse_current.right() == 0
    }

    /// ONLY after real 43c8e0 selection/arbitration/hover/dwell, with selected
    /// nonnull. Never choose a GUI recipient here. Native right suppresses this
    /// branch. The initial take for event10 happens before slot14 dispatch.
    pub fn prepare_selected_pointer_dispatch(
        &mut self,
        key_target_present: bool,
    ) -> Option<PreparedPointerDispatch> {
        let current = self.cells.mouse_current;
        let prior = self.cells.mouse_prior;
        if current.right() != 0 {
            return None;
        }
        let event = if current.left() != 0 {
            PointerEvent {
                code: if prior.left() != 0 { 5 } else { 4 },
                value: 0,
            }
        } else if prior.left() != 0 {
            PointerEvent { code: 6, value: 0 }
        } else if current.wheel() > 0 && prior.wheel() == 0 {
            PointerEvent { code: 8, value: 0 }
        } else if current.wheel() < 0 && prior.wheel() == 0 {
            PointerEvent { code: 9, value: 0 }
        } else if !key_target_present && self.character_available_byte() != 0 {
            PointerEvent {
                code: 10,
                value: self.take_character(),
            }
        } else {
            return None;
        };
        Some(PreparedPointerDispatch { event })
    }

    /// The native rereads group.key_target AFTER the real dispatch. Callback
    /// may change it and/or the CHAR storage. Even a wrapper-dropped event has
    /// returned from dispatch. Calling this does not certify handler success.
    pub fn finish_selected_pointer_dispatch(
        &mut self,
        _ticket: PreparedPointerDispatch,
        key_target_present_now: bool,
    ) -> Option<u8> {
        if key_target_present_now {
            None
        } else {
            Some(self.take_character())
        }
    }

    /// Separate 43cb0b tail: slot14(10,0,0) on a concrete key target whose +1c
    /// byte equals 1. The target owns taking chars; this predicate does not.
    pub fn key_target_char_admitted(&self, target_byte_1c: u8) -> bool {
        target_byte_1c == 1 && self.character_available_byte() != 0
    }

    /// Only the input cell slice of focus loss/gain. Caller owns receiver-null
    /// guard, live device receiver reloads and 0,1 unacquire/acquire, frontend
    /// callbacks and signed pause counters/mask2. These methods claim none of
    /// those external actions succeeded. No packet/queue/override clears.
    pub fn focus_loss_cells_after_device_calls(&mut self) {
        self.cells.input_enabled = 0;
    }
    pub fn focus_gain_cells_after_device_calls(&mut self) {
        self.cells.focus_transition = 1;
        self.cells.input_enabled = 1;
    }

    /// WndProc WM_CLOSE/WM_DESTROY clear this byte (4c6ad5/4c6aed).
    pub fn window_closing_transition_cell(&mut self) {
        self.cells.focus_transition = 0;
    }
}

#[cfg(test)]
#[path = "native_input/tests.rs"]
mod tests;
