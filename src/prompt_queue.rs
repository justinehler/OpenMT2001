//! Original prompt text wrapping (4e7a90), retained separately from host input.
pub fn wrap(tab: &[u8], text: &[u8], width: i32) -> Result<(Vec<usize>, Vec<u8>), String> {
    wrap_with_gap(tab, text, width, 0)
}
/// Original4e7b35..4e7b3f applies font-manager5ec before scaling the advance.
/// Existing gap-zero native observations retain their explicit contract.
pub fn wrap_with_gap(
    tab: &[u8],
    text: &[u8],
    width: i32,
    gap: i32,
) -> Result<(Vec<usize>, Vec<u8>), String> {
    crate::font::Font::parse(tab)?;
    if width <= 0 {
        return Err("prompt width must be positive".into());
    }
    let mut offsets = vec![0];
    let mut pos = 0;
    while pos < text.len() {
        let mut advance = 0i32;
        let mut last_space = None;
        let mut forced = None;
        while pos < text.len() {
            let at = 4 + usize::from(text[pos]) * 44 + 32;
            let metric = i32::from_le_bytes(
                tab.get(at..at + 4)
                    .ok_or("prompt glyph range")?
                    .try_into()
                    .unwrap(),
            );
            advance = advance
                .checked_add(metric.wrapping_add(gap))
                .ok_or("prompt width overflow")?;
            if advance >= width && last_space.is_some() {
                break;
            }
            pos += 1;
            if text.get(pos) == Some(&0) {
                last_space = Some(pos);
            }
            if text.get(pos..pos + 2) == Some(&[28, 46]) {
                forced = Some(pos + 2);
                break;
            }
        }
        if let Some(next) = forced.or_else(|| {
            if advance >= width {
                last_space.map(|p| p + 1)
            } else {
                None
            }
        }) {
            pos = next;
            offsets.push(pos);
        } else {
            offsets.push(text.len() + 1);
            break;
        }
    }
    let mut copy = text.to_vec();
    for i in 0..copy.len().saturating_sub(1) {
        if copy[i] == 28 {
            copy[i] = 0;
        }
    }
    Ok((offsets, copy))
}

/// A single original modal request, including script messages and prompts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub script: Option<ScriptText>,
    pub dialog: Option<crate::human_dialog::Dialog>,
    pub title: u32,
    pub body: u32,
    pub dismissal_id: u32,
    pub card: Option<CardText>,
}
/// Original428b30 reopens the active level's briefing in the generic panel.
/// Skirmish titles use the selected setup target, rather than the live score.
pub fn scenario_reminder(
    level: &crate::level_catalog::Level,
    network: bool,
    objective: i32,
) -> Request {
    use crate::human_dialog::{Argument, Dialog};
    let mut dialog = Dialog {
        buttons: [1437, 0],
        button_count: 1,
        ..Default::default()
    };
    let mut script = None;
    match level.labels() {
        crate::level_catalog::Labels::Localized { title, description } => {
            dialog.title = title;
            dialog.body = description;
            if (network || !level.single_player()) && (1..=6).contains(&objective) {
                dialog.title = 1820 + objective as u32;
                let target = level.objective_target() as i32;
                dialog.title_argument = Some(if matches!(objective, 1 | 4) {
                    Argument::Currency(target)
                } else {
                    Argument::Integer(target)
                });
            }
        }
        crate::level_catalog::Labels::Literal { title, description } => {
            script = Some(ScriptText {
                title: Some(title.into()),
                body: Some(description.into()),
                format_value: 0,
                timeout: true,
            });
        }
    }
    Request {
        script,
        dialog: Some(dialog),
        title: dialog.title,
        body: dialog.body,
        dismissal_id: 0,
        card: None,
    }
}
/// Owned independently for every queued message; numeric fields retain their IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptText {
    pub title: Option<std::sync::Arc<[u8]>>,
    pub body: Option<std::sync::Arc<[u8]>>,
    pub format_value: u32,
    /// Tutorial script messages explicitly disable the modal timeout.
    pub timeout: bool,
}
/// Bounded native no-argument printf policy, followed by original row0 mapping.
pub fn literal_glyphs(raw: &[u8], row: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut i = 0;
    while i < raw.len() && raw[i] != 0 {
        let b = raw[i];
        if b < 32 && b != 10 {
            break;
        }
        if b == b'%' {
            if raw.get(i + 1) != Some(&b'%') {
                return Err(
                    "Literal prompt argument-consuming or incomplete format is unsupported".into(),
                );
            }
            i += 1;
        }
        if b == 10 {
            for index in [0x3c, 0x4e] {
                bytes.push(*row.get(index).ok_or("prompt language map truncated")?);
            }
        } else {
            bytes.push(
                *row.get(usize::from(b - 32))
                    .ok_or("prompt language map truncated")?,
            );
        }
        i += 1;
    }
    Ok(bytes)
}

pub fn script_field(
    request: &Request,
    field: usize,
    strings: &[Vec<u8>],
) -> Result<Option<Vec<Vec<u8>>>, String> {
    let Some(script) = &request.script else {
        return Ok(None);
    };
    let raw = if field == 0 {
        &script.title
    } else {
        &script.body
    };
    if let Some(raw) = raw {
        return Ok(Some(vec![literal_glyphs(
            raw,
            strings.first().ok_or("prompt map missing")?,
        )?]));
    }
    let id = if field == 0 {
        request.title
    } else {
        request.body
    };
    let text = strings
        .get(id as usize)
        .ok_or("prompt language ID out of range")?;
    let argument = crate::prompt_format::Argument::Word(script.format_value);
    let output = crate::prompt_format::resource(text, strings, &[argument]);
    // The original row bridge displays even a failed gatherer's emitted prefix.
    Ok(Some(vec![output.glyphs]))
}
/// Original41c88f..41c966 presentation data. Names remain original ASCII until
/// insertion into the encoded language text; the amount preserves its float bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardText {
    pub portrait: u32,
    pub actor: [u8; 96],
    pub amount: Option<u32>,
}

pub fn card_request(
    image: &crate::pe::PeImage<'_>,
    actor: &[u8],
    notice: crate::event_cards::effects::Notice,
) -> Result<Request, String> {
    let word = |at| {
        actor
            .get(at..at + 4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .ok_or("card notice player truncated")
    };
    let character = word(0x150)?;
    if character >= 10 {
        return Err("card notice character outside original portrait registry".into());
    }
    let row = crate::event_cards::lifecycle::card_row(image, notice.card)?;
    let text = u32::from_le_bytes(row[4..8].try_into().unwrap());
    Ok(Request {
        script: None,
        dialog: None,
        title: 1488,
        body: text.wrapping_add(1),
        dismissal_id: 0,
        card: Some(CardText {
            portrait: u32::from_le_bytes(
                image.at(0x541afc + character * 40, 4)?.try_into().unwrap(),
            ),
            actor: actor
                .get(0x158..0x1b8)
                .ok_or("card notice name truncated")?
                .try_into()
                .unwrap(),
            amount: notice.affected.then_some(notice.amount.abs().to_bits()),
        }),
    })
}
#[derive(Clone, Default)]
pub struct Slot {
    pub request: Option<Request>,
    pub active: bool,
    pub next: Option<usize>,
    pub remaining: i32,
    keys_armed: bool,
}
#[derive(Default)]
pub struct State {
    pub literal_body: Option<Vec<u8>>,
    dialog_replies: Vec<crate::human_dialog::Reply>,
    pub pressed_dialog: Option<usize>,
    pub slots: [Slot; 4],
    pub head: Option<usize>,
    pub last: Option<usize>,
    pub cursor: [f32; 2],
    pub pressed: bool,
    last_tick: Option<u64>,
    deferred: bool,
    timeout_pending: bool,
    pending_backspace: bool,
    /// Backspace still owed to a notice that Return's deferred callback
    /// exposed; applied on the next UI event1.
    carried_backspace: bool,
}
impl State {
    /// Retained same-target left-button capture, released by the actual prompt
    /// input producer. A visible prompt alone does not capture the pointer.
    pub fn input_captured(&self) -> bool {
        self.pressed || self.pressed_dialog.is_some()
    }
    pub fn request(&self) -> Option<Request> {
        self.head.and_then(|i| self.slots[i].request.clone())
    }
    /// An expired modal stops drawing immediately, while its slot stays owned
    /// until the worker has run the continuation and admitted its requests.
    pub fn visible(&self) -> bool {
        self.head.is_some() && !self.timeout_pending
    }
    /// Tutorial messages leave controls and picking to the independent script gates.
    pub fn blocks_controls(&self) -> bool {
        self.request().is_some_and(|r| {
            !(r.script.is_some_and(|s| !s.timeout) && r.dialog.is_some_and(|d| d.button_count == 0))
        })
    }
    /// Apply script closes at their position among newly created requests.
    pub fn apply_requests(&mut self, requests: Vec<Request>, closes: &[usize]) {
        let mut closes = closes.iter().copied().peekable();
        let count = requests.len();
        for (index, request) in requests.into_iter().enumerate() {
            while closes.peek() == Some(&index) {
                self.dismiss_with(|_, _| {});
                closes.next();
            }
            self.submit(request);
        }
        while closes.peek() == Some(&count) {
            self.dismiss_with(|_, _| {});
            closes.next();
        }
    }
    /// Original429b50 reuses the first free slot. A full queue overwrites the
    /// last-created request; return that replaced payload for host diagnostics.
    pub fn submit(&mut self, mut request: Request) -> Option<Request> {
        let free = self.slots.iter().position(|s| !s.active);
        let at = free.or(self.last).expect("full prompt queue has last slot");
        let replaced = if free.is_none() {
            self.slots[at].request.clone()
        } else {
            None
        };
        if let (Some(previous), Some(dialog)) = (replaced.as_ref(), request.dialog.as_mut()) {
            //429b50's full-queue reuse skips the callback/context/timeout reset.
            //Only fields explicitly written by this message replace those words.
            if !dialog.handler_written {
                dialog.callback = previous.dialog.map_or(0, |d| d.callback);
            }
            if !dialog.context_written {
                dialog.context = previous.dialog.map_or(previous.dismissal_id, |d| d.context);
            }
            if !dialog.timeout_written {
                dialog.timeout = previous.dialog.is_none_or(|d| d.timeout);
            }
        }
        if free.is_some() {
            self.slots[at] = Slot {
                request: None,
                active: true,
                next: None,
                remaining: 1000,
                keys_armed: false,
            };
        }
        if let Some(ticks) = request.dialog.and_then(|d| d.timeout_ticks) {
            self.slots[at].remaining = ticks;
        }
        self.last = Some(at);
        if let Some(mut tail) = self.head {
            while let Some(next) = self.slots[tail].next {
                tail = next;
            }
            if tail != at {
                self.slots[tail].next = Some(at);
            }
        } else {
            self.head = Some(at);
        }
        self.slots[at].request = Some(request);
        self.slots[at].keys_armed = false;
        replaced
    }
    pub fn cursor(&mut self, point: [f32; 2]) {
        self.cursor = point;
    }
    /// Normal scenarios retain429b50's1000-tick timeout.42a1f0 checks zero
    /// before decrementing; queued slots only age while they are at the head.
    pub fn tick(&mut self) -> Option<u32> {
        if self.deferred {
            return None;
        }
        let at = self.head?;
        if let Some(dialog) = self.slots[at].request.as_ref()?.dialog {
            if dialog.timeout {
                if self.slots[at].remaining <= 0 {
                    self.dialog_reply(0);
                    self.timeout_pending = true;
                } else {
                    self.slots[at].remaining -= 1;
                }
            }
            return None;
        }
        if self.slots[at]
            .request
            .as_ref()?
            .script
            .as_ref()
            .is_some_and(|s| !s.timeout)
        {
            return None;
        }
        if self.slots[at].remaining <= 0 {
            let callback = self.begin_dismiss();
            self.timeout_pending = self.deferred;
            callback
        } else {
            self.slots[at].remaining -= 1;
            None
        }
    }
    /// Keep the old slot active until a worker callback has submitted its own
    /// requests. This preserves the original full-queue overwrite ordering.
    pub fn begin_dismiss(&mut self) -> Option<u32> {
        if self.deferred {
            return None;
        }
        if self.request()?.dialog.is_some() {
            return None;
        }
        let id = self.request()?.dismissal_id;
        if id == 0 {
            self.dismiss_with(|_, _| {});
            None
        } else {
            self.deferred = true;
            Some(id)
        }
    }
    pub fn finish_dismiss(&mut self, requests: Vec<Request>) {
        for request in requests {
            self.submit(request);
        }
        self.deferred = false;
        self.dismiss_with(|_, _| {});
        // Resume the independent second branch of the same event only after
        // Return's worker callback and close have exposed the resulting head.
        if std::mem::take(&mut self.pending_backspace) {
            match self.request() {
                Some(request) if request.dialog.is_some() => {
                    self.dialog_reply(1);
                }
                // A notice with a script callback needs its id handed to the
                // worker, which this call can't return; dismiss it on the next
                // UI event1 instead.
                Some(request) if request.dismissal_id != 0 => self.carried_backspace = true,
                Some(_) => {
                    self.dismiss_with(|_, _| {});
                }
                None => {}
            }
        }
    }
    /// 42a1f0's Backspace test acts on whatever head is current at that point,
    /// including one Return's dismissal just exposed, without re-arming. With
    /// no head it does nothing (the original reads a null head there).
    fn backspace_head(&mut self) -> Option<u32> {
        if self.request()?.dialog.is_some() {
            self.dialog_reply(1);
            None
        } else {
            self.begin_dismiss()
        }
    }
    pub fn request_left_button(&mut self, down: bool, bounds: [f32; 4]) -> Option<u32> {
        if down {
            self.left_button(true, bounds);
            None
        } else {
            let hit = self.cursor[0] >= bounds[0]
                && self.cursor[0] <= bounds[2]
                && self.cursor[1] >= bounds[1]
                && self.cursor[1] <= bounds[3];
            let activate = self.pressed && hit;
            self.pressed = false;
            if activate { self.begin_dismiss() } else { None }
        }
    }
    /// Consume the worker's cumulative delivered group updates. Keep updates
    /// after a deferred callback for the next head, rather than dropping them.
    pub fn advance_to(&mut self, delivered: u64) -> Vec<u32> {
        let mut callbacks = Vec::new();
        let mut consumed = self.last_tick.unwrap_or(delivered);
        while consumed < delivered && !self.deferred {
            consumed += 1;
            if let Some(id) = self.tick() {
                callbacks.push(id);
            }
        }
        self.last_tick = Some(consumed);
        callbacks
    }
    pub fn cancel_press(&mut self) {
        self.pressed = false;
        self.pressed_dialog = None;
    }
    pub fn take_dialog_replies(&mut self) -> Vec<crate::human_dialog::Reply> {
        std::mem::take(&mut self.dialog_replies)
    }
    /// Original42a1f0 arms on the first UI event1. Later Return/Backspace
    /// edges reply independently of the number of visible mouse buttons.
    ///
    /// A notice (a request without a dialog) is dismissed by the same armed
    /// Return or Backspace edge, as every prompt kind in 42a1f0 is. When that
    /// dismissal has a script callback, its id is returned and the host sends
    /// it to the worker, as for a click.
    pub fn dialog_event1(&mut self, edges: [bool; 2]) -> Option<u32> {
        let at = self.head?;
        if self.deferred {
            return None;
        }
        if std::mem::take(&mut self.carried_backspace) {
            return self.backspace_head();
        }
        let notice = self.slots[at].request.as_ref()?.dialog.is_none();
        if !std::mem::replace(&mut self.slots[at].keys_armed, true) {
            return None;
        }
        if notice {
            if !edges[0] {
                return if edges[1] { self.begin_dismiss() } else { None };
            }
            let callback = self.begin_dismiss();
            if !edges[1] {
                return callback;
            }
            // Return and Backspace in one event: Backspace then dismisses the
            // head Return exposed, once Return's callback has finished.
            if self.deferred {
                self.pending_backspace = true;
                return callback;
            }
            return self.backspace_head();
        }
        if edges[0] {
            self.pending_backspace = edges[1];
            self.dialog_reply(0);
        } else if edges[1] {
            self.dialog_reply(1);
        }
        None
    }
    pub fn dialog_button(&mut self, button: usize) {
        if self
            .request()
            .and_then(|r| r.dialog)
            .is_some_and(|d| button < d.button_count)
        {
            self.dialog_reply(button);
        }
    }
    fn dialog_reply(&mut self, button: usize) {
        if self.deferred {
            return;
        }
        let Some(dialog) = self.request().and_then(|r| r.dialog) else {
            return;
        };
        self.deferred = true;
        self.dialog_replies.push(crate::human_dialog::Reply {
            callback: dialog.callback,
            context: dialog.context,
            button: button as u32,
        });
        self.pressed_dialog = None;
    }
    pub fn dialog_mouse(&mut self, down: bool, buttons: &[[f32; 4]]) {
        let hit = buttons.iter().position(|b| {
            self.cursor[0] >= b[0]
                && self.cursor[0] <= b[2]
                && self.cursor[1] >= b[1]
                && self.cursor[1] <= b[3]
        });
        if down {
            self.pressed_dialog = hit;
        } else if let Some(button) = self
            .pressed_dialog
            .take()
            .filter(|button| Some(*button) == hit)
        {
            self.dialog_button(button);
        }
    }
    /// Original429d50 fires the nonzero callback before unlinking the head.
    /// A callback may submit more prompts while the dismissed slot is active.
    pub fn dismiss_with(&mut self, callback: impl FnOnce(&mut Self, u32)) -> Option<u32> {
        let id = self.request()?.dismissal_id;
        if id != 0 {
            callback(self, id);
        }
        if let Some(at) = self.head {
            self.slots[at].active = false;
            self.head = self.slots[at]
                .next
                .take()
                .filter(|&next| self.slots[next].active);
        }
        self.pressed = false;
        self.timeout_pending = false;
        (id != 0).then_some(id)
    }
    /// Host shortcut when dismissal dispatch cannot reenter the presenter.
    pub fn left_button(&mut self, down: bool, bounds: [f32; 4]) -> Option<u32> {
        self.left_button_with(down, bounds, |_, _| {})
    }
    pub fn left_button_with(
        &mut self,
        down: bool,
        bounds: [f32; 4],
        callback: impl FnOnce(&mut Self, u32),
    ) -> Option<u32> {
        let hit = self.request().is_some()
            && self.cursor[0] >= bounds[0]
            && self.cursor[0] <= bounds[2]
            && self.cursor[1] >= bounds[1]
            && self.cursor[1] <= bounds[3];
        if down {
            self.pressed = hit;
            None
        } else {
            let activate = self.pressed && hit;
            self.pressed = false;
            if activate {
                self.dismiss_with(callback)
            } else {
                None
            }
        }
    }
}

#[cfg(feature = "lua4")]
pub fn script_request(
    arguments: &[crate::lua4::Value],
    tutorial: bool,
    continue_button: bool,
) -> Result<Request, String> {
    let number = |at: usize| match arguments.get(at) {
        Some(crate::lua4::Value::Number(value)) => crate::x87::Sum::from_f64(*value)
            .map(|v| v.ftol_low_word() as u32)
            .map_err(str::to_owned),
        None | Some(crate::lua4::Value::Nil) if at >= 2 => Ok(0),
        _ => Err("Prompt requires C-coerced numeric context/value".to_owned()),
    };
    let field = |at| match arguments.get(at) {
        Some(crate::lua4::Value::Number(_)) => Ok((number(at)?, None)),
        Some(crate::lua4::Value::Bytes(bytes)) => {
            // Validate safe formatting now; row0 mapping is deferred to rendering.
            literal_glyphs(bytes, &(0..224).map(|i| i as u8).collect::<Vec<_>>())?;
            Ok((0, Some(std::sync::Arc::<[u8]>::from(bytes.clone()))))
        }
        _ => Err("Prompt title/body must be numeric or convertible to string".to_owned()),
    };
    let (title, literal_title) = field(0)?;
    let (body, literal_body) = field(1)?;
    Ok(Request {
        script: Some(ScriptText {
            title: literal_title,
            body: literal_body,
            format_value: number(3)?,
            timeout: !tutorial,
        }),
        dialog: Some(crate::human_dialog::Dialog {
            title,
            body,
            buttons: [1707, 0],
            button_count: usize::from(continue_button),
            context: number(2)?,
            context_written: true,
            timeout: !tutorial,
            timeout_written: tutorial,
            ..Default::default()
        }),
        card: None,
        title,
        body,
        dismissal_id: number(2)?,
    })
}

#[cfg(test)]
mod modal_update_tests {
    use super::*;

    fn timed(remaining: i32, timeout: bool, context: u32) -> Request {
        Request {
            script: None,
            dialog: Some(crate::human_dialog::Dialog {
                timeout_ticks: Some(remaining),
                timeout,
                context,
                context_written: true,
                timeout_written: true,
                ..Default::default()
            }),
            title: 0,
            body: 0,
            dismissal_id: 0,
            card: None,
        }
    }

    #[test]
    fn modal_timeout_expires_on_n_plus_one_even_without_buttons() {
        for remaining in [1, 3, 1000] {
            let mut state = State::default();
            state.advance_to(70);
            state.submit(timed(remaining, true, 17));
            for delivered in 1..=remaining as u64 {
                assert!(state.advance_to(70 + delivered).is_empty());
                assert!(state.take_dialog_replies().is_empty());
            }
            state.advance_to(71 + remaining as u64);
            assert_eq!(state.take_dialog_replies()[0].context, 17);
            assert!(state.request().is_some(), "callback before close");
            state.finish_dismiss(vec![]);
            assert!(state.request().is_none());
        }
    }

    #[test]
    fn modal_timeout_zero_policy_and_suppressed_updates_do_not_expire() {
        let mut state = State::default();
        state.advance_to(10);
        state.submit(timed(2, true, 17));
        for _ in 0..1002 {
            state.advance_to(10); // No delivered group updates during suppression.
        }
        state.advance_to(12);
        assert!(state.take_dialog_replies().is_empty());
        state.advance_to(13);
        assert_eq!(state.take_dialog_replies()[0].button, 0);
        state.finish_dismiss(vec![]);
        let mut dialog = crate::human_dialog::Dialog::default();
        dialog
            .outcome(crate::outcome_notice::Call::Timeout(0))
            .unwrap();
        let mut request = timed(0, true, 29);
        request.dialog = Some(dialog);
        state.submit(request);
        state.advance_to(2013);
        assert!(state.take_dialog_replies().is_empty());
        assert!(state.request().is_some());
    }

    #[test]
    fn modal_timeout_batch_resumes_on_next_head_after_worker_reply() {
        let mut state = State::default();
        state.advance_to(0);
        state.submit(timed(1, true, 17));
        state.submit(timed(1, true, 29));
        state.advance_to(4);
        assert_eq!(state.take_dialog_replies()[0].context, 17);
        state.finish_dismiss(vec![]);
        state.advance_to(4);
        assert_eq!(state.take_dialog_replies()[0].context, 29);
        state.finish_dismiss(vec![]);
        assert!(state.request().is_none());
    }

    #[test]
    #[cfg(feature = "lua4")]
    fn tutorial_script_request_survives_until_its_continuation_dismisses_it() {
        let arguments: [_; 3] = std::array::from_fn(|_| crate::lua4::Value::Number(1.));
        for tutorial in [true, false] {
            let mut state = State::default();
            state.advance_to(0);
            let request = script_request(&arguments, tutorial, true).unwrap();
            state.submit(request);
            assert!(state.advance_to(1002).is_empty());
            if tutorial {
                assert!(state.take_dialog_replies().is_empty());
                state.dialog_event1([false; 2]);
                state.dialog_event1([true, false]);
            }
            assert_eq!(state.take_dialog_replies()[0].context, 1);
            assert!(state.request().is_some());
            state.finish_dismiss(vec![]);
            assert!(state.request().is_none());
        }
    }
}
