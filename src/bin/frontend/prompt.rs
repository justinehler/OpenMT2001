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
    openmt2001::font::Font::parse(tab)?;
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

use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

/// A single original Interface.CreatePrompt request.
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
    level: &openmt2001::level_catalog::Level,
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
        openmt2001::level_catalog::Labels::Localized { title, description } => {
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
        openmt2001::level_catalog::Labels::Literal { title, description } => {
            script = Some(ScriptText {
                title: Some(title.into()),
                body: Some(description.into()),
                format_value: 0,
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
/// Proven resource subset of4e7fe0/4e80d5: one signed32-bit decimal word.
/// Parse atomically into private output; return no text on unsupported formats.
fn resource_integer(text: &[u8], row: &[u8], word: u32) -> Result<Vec<u8>, String> {
    let selector = |at| row.get(at).copied().ok_or("prompt map truncated");
    let percent = selector(5)?;
    let d = selector(0x44)?;
    let integer = selector(0x49)?;
    let mut result = Vec::with_capacity(text.len());
    let mut used = false;
    let mut at = 0;
    while at < text.len() {
        if text[at] != percent {
            result.push(text[at]);
            at += 1;
            continue;
        }
        let next = *text
            .get(at + 1)
            .ok_or("resource prompt trailing percent is unsupported")?;
        if next != d && next != integer {
            return Err(
                "resource prompt pointer, escape or unproved directive is unsupported".into(),
            );
        }
        if used {
            return Err("resource prompt consumes more than one word".into());
        }
        used = true;
        for byte in (word as i32).to_string().bytes() {
            result.push(
                *row.get(usize::from(byte - 32))
                    .ok_or("prompt decimal glyph missing")?,
            );
        }
        at += 2;
    }
    Ok(result)
}

fn script_field(
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
    Ok(Some(vec![resource_integer(
        text,
        strings.first().ok_or("prompt map missing")?,
        script.format_value,
    )?]))
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
    image: &openmt2001::pe::PeImage<'_>,
    actor: &[u8],
    notice: openmt2001::event_cards::effects::Notice,
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
    let row = openmt2001::event_cards::lifecycle::card_row(image, notice.card)?;
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
struct Slot {
    request: Option<Request>,
    active: bool,
    next: Option<usize>,
    remaining: i32,
    keys_armed: bool,
}
#[derive(Default)]
pub struct State {
    literal_body: Option<Vec<u8>>,
    dialog_replies: Vec<crate::human_dialog::Reply>,
    pressed_dialog: Option<usize>,
    slots: [Slot; 4],
    head: Option<usize>,
    last: Option<usize>,
    cursor: [f32; 2],
    pressed: bool,
    last_tick: Option<u64>,
    deferred: bool,
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
                    self.dialog_button(0);
                } else {
                    self.slots[at].remaining -= 1;
                }
            }
            return None;
        }
        if self.slots[at].remaining <= 0 {
            self.begin_dismiss()
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
    pub fn advance_to(&mut self, tick: u64) -> Vec<u32> {
        let mut callbacks = Vec::new();
        let previous = self.last_tick.replace(tick).unwrap_or(tick);
        for _ in previous..tick {
            if let Some(id) = self.tick() {
                callbacks.push(id);
            }
        }
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
    /// edges use the same reply path as the corresponding mouse button.
    pub fn dialog_event1(&mut self, edges: [bool; 2]) {
        let Some(at) = self.head else {
            return;
        };
        if self.deferred
            || self.slots[at]
                .request
                .as_ref()
                .is_none_or(|r| r.dialog.is_none())
        {
            return;
        }
        if !std::mem::replace(&mut self.slots[at].keys_armed, true) {
            return;
        }
        for (button, edge) in edges.into_iter().enumerate() {
            if edge {
                self.dialog_button(button);
            }
        }
    }
    pub fn dialog_button(&mut self, button: usize) {
        if self.deferred {
            return;
        }
        let Some(dialog) = self.request().and_then(|r| r.dialog) else {
            return;
        };
        if button >= dialog.button_count {
            return;
        }
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

fn substitute(text: &[u8], value: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut at = 0;
    while at < text.len() {
        if text[at..].starts_with(&[b'%' - 32, b's' - 32]) {
            result.extend_from_slice(value);
            at += 2;
        } else {
            result.push(text[at]);
            at += 1;
        }
    }
    result
}
fn card_body(request: &Request, strings: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
    let card = request.card.ok_or("missing card presentation")?;
    let end = card
        .actor
        .iter()
        .position(|b| *b == 0)
        .ok_or("unterminated card actor name")?;
    let name = card.actor[..end]
        .iter()
        .map(|b| {
            if (32..=126).contains(b) {
                Ok(b - 32)
            } else {
                Err("card actor needs an unrecovered glyph mapping")
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut body = vec![substitute(
        strings
            .get(request.body as usize)
            .ok_or("card body language missing")?,
        &name,
    )];
    if let Some(bits) = card.amount {
        let amount = f32::from_bits(bits);
        if !amount.is_finite() {
            return Err("non-finite card notification amount".into());
        }
        //43a950 formats the stored absolute float with two decimal places.
        //4c1496 obtains currency placement/symbol from these language entries.
        let value: Vec<_> = ui::decimal2(amount)?.bytes().map(|b| b - 32).collect();
        let symbol = strings.get(2243).ok_or("card currency symbol missing")?;
        let left = strings.get(2244).and_then(|s| s.first()) == Some(&0x4c);
        let currency = if left {
            [symbol.as_slice(), value.as_slice()].concat()
        } else {
            [value.as_slice(), symbol.as_slice()].concat()
        };
        body.push(substitute(
            strings
                .get(request.body.wrapping_add(1) as usize)
                .ok_or("card affected language missing")?,
            &currency,
        ));
    }
    Ok(body)
}

/// Original4406a0 styles3/4 and4419f0: retain W's measured line height,
/// independent integer origins/offsets, and complete glyphs. Prompt text does
/// not enable the optional renderer clipping rectangle (+1b0 remains zero).
fn text_quads(
    tab: &[u8],
    paragraphs: &[Vec<u8>],
    bounds: [f32; 4],
    center: bool,
    x_margin: i32,
    y_margin: i32,
    parent_offset: [f32; 2],
) -> Result<Vec<openmt2001::font::GlyphQuad>, String> {
    let font = openmt2001::font::Font::parse(tab)?;
    let height_at = 4 + usize::from(b'W' - 32) * 44 + 36;
    let height = i32::from_le_bytes(
        tab.get(height_at..height_at + 4)
            .ok_or("prompt W metric missing")?
            .try_into()
            .unwrap(),
    );
    let mut rows = Vec::new();
    for text in paragraphs {
        let (offsets, copy) = wrap_with_gap(
            tab,
            text,
            (f64::from(bounds[2]) - f64::from(bounds[0]) - f64::from(x_margin) * 2.).trunc() as i32,
            -1,
        )?;
        for pair in offsets.windows(2) {
            let end = pair[1].saturating_sub(1).min(copy.len());
            rows.push(copy[pair[0].min(end)..end].to_vec());
        }
    }
    let top = (f64::from(bounds[1]) + f64::from(y_margin)).trunc() as f32
        + ((f64::from(bounds[3])
            - f64::from(bounds[1])
            - rows.len().max(1) as f64 * f64::from(height))
            * 0.5)
            .trunc() as f32;
    let mut quads = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let width = ui::text_width(tab, row, 1.)?;
        let left = bounds[0].trunc()
            + if center {
                ((f64::from(bounds[2]) - f64::from(bounds[0]) - f64::from(width)
                    + f64::from(x_margin))
                    * 0.5
                    + 0.5)
                    .trunc() as f32
            } else {
                (x_margin + 2) as f32
            };
        quads.extend(font.layout(
            row,
            openmt2001::font::Layout {
                origin: [
                    left + parent_offset[0].trunc(),
                    top + index as f32 * height as f32 + parent_offset[1].trunc(),
                ],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )?);
    }
    Ok(quads)
}

/// Native43b5f0 computes the root hit rectangle from every vertex in the
/// active mesh frame, including backing edges outside the named text regions.
fn parent_input_bounds(bytes: &[u8]) -> Result<[f32; 4], String> {
    let mesh = openmt2001::mesh::CachedMesh::parse(bytes)?;
    openmt2001::ui_mesh::parent_bounds(&mesh, 0)
}
fn parent_input_hit(bounds: [f32; 4], pointer: [f32; 2], offset: [f32; 2]) -> bool {
    openmt2001::ui_mesh::parent_hit(bounds, pointer, offset)
}
pub struct Assets {
    main: [f32; 4],
    input_bounds: [f32; 4],
    base: Vec<Surface>,
    portrait: Surface,
    card_portraits: Vec<Surface>,
    buttons: Vec<Surface>,
    fonts: Vec<Vec<Surface>>,
    tabs: Vec<Vec<u8>>,
    strings: Vec<Vec<u8>>,
    title: [f32; 4],
    body: [f32; 4],
    pub button: [f32; 4],
}
impl Assets {
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        let (mut base, regions) = ui::panel_base(
            index,
            archive,
            "gamedata2/brand_new_panels/prompt_panel.tm",
            // Only the border is visible model geometry; parts1/2 are
            // reverse-facing layout regions replaced by native runtime fills.
            &[0],
        )?;
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index.find(name).ok_or("prompt asset missing")?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("prompt asset range".into())
        };
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let input_bounds =
            parent_input_bounds(payload("gamedata2/brand_new_panels/prompt_panel.tm")?)?;
        let mut portrait =
            Surface::new(generic.textures.get(302).ok_or("prompt portrait missing")?);
        textured_rect(&mut portrait, regions[3]);
        let card_portraits = generic
            .textures
            .get(263..273)
            .ok_or("card portraits missing")?
            .iter()
            .map(|texture| {
                let mut surface = Surface::new(texture);
                textured_rect(&mut surface, regions[3]);
                surface
            })
            .collect();
        let buttons = generic.textures[79..82]
            .iter()
            .map(Surface::new)
            .collect::<Vec<_>>();
        let textures = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let fonts = vec![
            textures.textures[..2].iter().map(Surface::new).collect(),
            textures.textures[6..8].iter().map(Surface::new).collect(),
        ];
        let tabs = ["cust640p.tab", "tahoma10bldnoantip.tab"]
            .iter()
            .map(|n| fs::read(installer.join("Standard/fonts").join(n)).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let main = regions[2];
        // Original5254c0+10 ->42a150 draws this PE-backed body before
        // the model border;43c3a0 then draws429e30's title and children.
        let body_color = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        let mut backing = Surface::solid();
        backing.rect(main, [body_color; 4]);
        base.insert(0, backing);
        let y = ((f64::from(main[1]) + 3. * f64::from(main[3])) * 0.25 + 2.) as f32;
        let x = ((f64::from(main[2]) - f64::from(main[0]) - 60.) / 2. + f64::from(main[0])) as f32;
        Ok(Self {
            main,
            input_bounds,
            base,
            portrait,
            card_portraits,
            buttons,
            fonts,
            tabs,
            strings: crate::tooltip::localized_strings(installer, strings)?,
            title: regions[1],
            body: [regions[3][2], main[1], main[2], y],
            button: [x, y, x + 60., y + 24.],
        })
    }
    pub fn dialog_buttons(&self, count: usize) -> Vec<[f32; 4]> {
        crate::human_dialog::buttons(self.main, count)
    }
    /// 429f03..429f64 starts the tutorial arrow at the prompt widget centre.
    pub fn arrow_origin(&self) -> [f32; 2] {
        [
            (self.main[0] + self.main[2]) * 0.5,
            (self.main[1] + self.main[3]) * 0.5,
        ]
    }
    /// Native 4db740 stores the two PositionPrompt coordinates on the prompt
    /// widget. They translate its button and all rendered prompt surfaces.
    pub fn button_at(&self, offset: [f32; 2]) -> [f32; 4] {
        [
            self.button[0] + offset[0],
            self.button[1] + offset[1],
            self.button[2] + offset[0],
            self.button[3] + offset[1],
        ]
    }
    /// Original5254c0+24→43bca0→43bc70 tests the actual visible prompt
    /// parent's mesh rectangle before looking for an enabled button child.
    pub fn input_occupied(&self, pointer: [f32; 2], offset: [f32; 2]) -> bool {
        parent_input_hit(self.input_bounds, pointer, offset)
    }
    pub fn prepare(&self, renderer: &Renderer, state: &State) -> Result<PreparedScene, String> {
        self.prepare_positioned(renderer, state, [0.; 2])
    }
    /// Present a local original generic dialog without enrolling it in the
    /// Host prompt queue. Save Game owns its confirmation callback and uses
    /// this same 429e30 panel, typography and 429630 button geometry.
    pub fn prepare_error(
        &self,
        renderer: &Renderer,
        message: &str,
    ) -> Result<PreparedScene, String> {
        let mut state = State::default();
        state.submit(Request {
            script: None,
            dialog: None,
            title: 1488,
            body: 0,
            dismissal_id: 0,
            card: None,
        });
        state.literal_body = Some(error_glyphs(&self.strings, message)?);
        self.prepare(renderer, &state)
    }
    pub fn prepare_dialog(
        &self,
        renderer: &Renderer,
        dialog: crate::human_dialog::Dialog,
        cursor: [f32; 2],
        pressed_button: Option<usize>,
    ) -> Result<PreparedScene, String> {
        let mut state = State::default();
        state.submit(Request {
            script: None,
            dialog: Some(dialog),
            title: dialog.title,
            body: dialog.body,
            dismissal_id: 0,
            card: None,
        });
        state.cursor(cursor);
        state.pressed_dialog = pressed_button;
        self.prepare(renderer, &state)
    }
    pub fn prepare_positioned(
        &self,
        renderer: &Renderer,
        state: &State,
        offset: [f32; 2],
    ) -> Result<PreparedScene, String> {
        self.prepare_with_button(renderer, state, true, offset)
    }
    /// Interface.CreateMessage shares the prompt body but does not create the
    /// OK control that CreatePrompt appends.
    pub fn prepare_message(
        &self,
        renderer: &Renderer,
        state: &State,
    ) -> Result<PreparedScene, String> {
        self.prepare_message_positioned(renderer, state, [0.; 2])
    }
    pub fn prepare_message_positioned(
        &self,
        renderer: &Renderer,
        state: &State,
        offset: [f32; 2],
    ) -> Result<PreparedScene, String> {
        self.prepare_with_button(renderer, state, false, offset)
    }
    pub fn desktop_quit_hit_at(&self, cursor: [f32; 2], right: bool) -> Option<usize> {
        use crate::menu::desktop_quit;
        if right || !desktop_quit::hit(self.input_bounds, cursor) {
            return None;
        }
        self.dialog_buttons(2)
            .iter()
            .position(|bounds| desktop_quit::hit(*bounds, cursor))
    }
    pub fn desktop_quit_hit(&self, modal: &crate::menu::desktop_quit::Modal) -> Option<usize> {
        modal
            .pointer()
            .and_then(|p| self.desktop_quit_hit_at(p, modal.blocked_by_right()))
    }
    pub fn prepare_desktop_quit(
        &self,
        renderer: &Renderer,
        modal: &crate::menu::desktop_quit::Modal,
    ) -> Result<PreparedScene, String> {
        let mut state = State::default();
        state.submit(Request {
            script: None,
            dialog: Some(crate::menu::desktop_quit::dialog()),
            title: 1765,
            body: 1766,
            dismissal_id: 0,
            card: None,
        });
        state.cursor(modal.pointer().unwrap_or([-1.; 2]));
        state.pressed_dialog = modal.pressed_button();
        let surfaces =
            self.message_surfaces(&state, true, crate::menu::desktop_quit::OFFSET, Some(modal))?;
        self.prepare_surfaces(renderer, &surfaces)
    }
    fn prepare_with_button(
        &self,
        renderer: &Renderer,
        state: &State,
        show_button: bool,
        offset: [f32; 2],
    ) -> Result<PreparedScene, String> {
        let surfaces = self.message_surfaces(state, show_button, offset, None)?;
        self.prepare_surfaces(renderer, &surfaces)
    }
    fn message_surfaces(
        &self,
        state: &State,
        show_button: bool,
        offset: [f32; 2],
        desktop: Option<&crate::menu::desktop_quit::Modal>,
    ) -> Result<Vec<Surface>, String> {
        let request = state.request().ok_or("no active prompt")?;
        let mut surfaces = self.base.clone();
        // Original429e30 submits this title gradient before prompt strings.
        let mut title = Surface::solid();
        title.rect(self.title, [0xff909090, 0xff909090, 0x7f909090, 0x7f909090]);
        surfaces.push(title);
        let portrait = request
            .dialog
            .map(|d| d.portrait)
            .or_else(|| request.card.map(|c| c.portrait));
        if desktop.is_none() {
            surfaces.push(match portrait {
                Some(302) | None => self.portrait.clone(),
                Some(portrait) => self
                    .card_portraits
                    .get(portrait.wrapping_sub(263) as usize)
                    .ok_or("Modal portrait leaves original texture registry")?
                    .clone(),
            });
        }
        let buttons = if show_button {
            request
                .dialog
                .map(|d| self.dialog_buttons(d.button_count))
                .unwrap_or_else(|| vec![self.button])
        } else {
            Vec::new()
        };
        for (i, bounds) in buttons.iter().copied().enumerate() {
            let cursor = [state.cursor[0] - offset[0], state.cursor[1] - offset[1]];
            let hit = cursor[0] >= bounds[0]
                && cursor[0] <= bounds[2]
                && cursor[1] >= bounds[1]
                && cursor[1] <= bounds[3];
            let pressed = if request.dialog.is_some() {
                state.pressed_dialog == Some(i)
            } else {
                state.pressed
            };
            if let Some(modal) = desktop {
                let hover = self.desktop_quit_hit(modal) == Some(i);
                let mut button = Surface::solid();
                ui::button_state(
                    &mut button,
                    bounds,
                    crate::menu::desktop_quit::COLORS[i],
                    modal.button_state(i, hover),
                )?;
                surfaces.push(button);
                continue;
            }
            let mut button = self.buttons[if pressed && hit {
                2
            } else if hit {
                1
            } else {
                0
            }]
            .clone();
            textured_rect(&mut button, bounds);
            surfaces.push(button);
        }
        let mut text_fields = vec![
            (
                1,
                request.dialog.map_or(request.title, |d| d.title),
                self.title,
                true,
                4,
            ),
            (
                0,
                request.dialog.map_or(request.body, |d| d.body),
                if desktop.is_some() {
                    [
                        self.main[0],
                        self.main[1],
                        self.main[2],
                        ((3. * f64::from(self.main[3]) + f64::from(self.main[1])) * 0.25 + 2.)
                            as f32,
                    ]
                } else {
                    self.body
                },
                desktop.is_some(), // original no-portrait format(4,4); portrait path unchanged
                4,
            ),
        ];
        for (i, bounds) in buttons.into_iter().enumerate() {
            text_fields.push((
                1,
                request
                    .dialog
                    .map_or(if request.card.is_some() { 1437 } else { 1707 }, |d| {
                        d.buttons[i]
                    }),
                bounds,
                true,
                0,
            ));
        }
        //429e30 traverses the two retained prompt fields in reverse order:
        //body first, title second. Child button labels follow their parent.
        let mut text_fields = text_fields.into_iter().enumerate().collect::<Vec<_>>();
        text_fields.swap(0, 1);
        let text_surface_start = surfaces.len();
        for (field_index, (font, id, bounds, center, margin)) in text_fields {
            let scripted = if field_index < 2 {
                script_field(&request, field_index, &self.strings)?
            } else {
                None
            };
            let formatted = if scripted.is_some() {
                scripted
            } else if font == 0 && state.literal_body.is_some() {
                Some(vec![state.literal_body.clone().unwrap()])
            } else if field_index == 0
                && let Some(dialog) = request.dialog
            {
                Some(vec![dialog.title_text(&self.strings)?])
            } else if font == 0
                && let Some(dialog) = request.dialog
            {
                Some(dialog.paragraphs(&self.strings)?)
            } else if font == 0 && request.card.is_some() {
                Some(card_body(&request, &self.strings)?)
            } else {
                None
            };
            let ordinary = if formatted.is_none() {
                vec![
                    self.strings
                        .get(id as usize)
                        .ok_or("prompt language ID out of range")?
                        .clone(),
                ]
            } else {
                Vec::new()
            };
            let paragraphs = formatted.as_ref().unwrap_or(&ordinary);
            let mut pages = self.fonts[font].clone();
            for quad in text_quads(
                &self.tabs[font],
                paragraphs,
                bounds,
                center,
                margin,
                margin,
                offset,
            )? {
                pages
                    .get_mut(quad.page)
                    .ok_or("prompt glyph page missing")?
                    .quad(
                        quad.positions.map(|p| [p[0], p[1]]),
                        quad.uv,
                        [0xffffffff; 4],
                    );
            }
            surfaces.extend(pages);
        }
        for surface in &mut surfaces[..text_surface_start] {
            for vertex in &mut surface.vertices {
                vertex[0] += offset[0];
                vertex[1] += offset[1];
            }
        }
        Ok(surfaces)
    }
    fn prepare_surfaces(
        &self,
        renderer: &Renderer,
        surfaces: &[Surface],
    ) -> Result<PreparedScene, String> {
        let draws = surfaces.iter().map(Surface::draw).collect::<Vec<_>>();
        renderer.prepare(
            &draws,
            [
                2. / 640.,
                0.,
                0.,
                0.,
                0.,
                -2. / 480.,
                0.,
                0.,
                0.,
                0.,
                1.,
                0.,
                -1.,
                1.,
                0.,
                1.,
            ],
            None,
        )
    }
}
fn textured_rect(surface: &mut Surface, b: [f32; 4]) {
    surface.quad(
        [[b[0], b[1]], [b[0], b[3]], [b[2], b[3]], [b[2], b[1]]],
        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
        [0xffffffff; 4],
    );
}

#[cfg(test)]
pub fn verify_card_panel(scene: &openmt2001::render_scene::Scene, player: &[u8; 440]) {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe_path = openmt2001::fixtures::path("EXE");
    let language = exe_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let offsets = fs::read(language.join("tycoon.off")).unwrap();
    let bytes = fs::read(language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings = offsets
        .chunks_exact(2)
        .map(|n| {
            let end = at + u16::from_le_bytes(n.try_into().unwrap()) as usize;
            let text = &bytes[at..end];
            at = end;
            text
        })
        .collect::<Vec<_>>();
    let assets = Assets::load(&index, &archive, installer, &strings, &image).unwrap();
    let mut actor = vec![0; 0x220];
    actor[4..444].copy_from_slice(player);
    let request = card_request(
        &image,
        &actor,
        openmt2001::event_cards::effects::Notice {
            player: 0,
            card: 16,
            enabled: true,
            affected: true,
            amount: -1234.125,
        },
    )
    .unwrap();
    let text = card_body(&request, &assets.strings).unwrap();
    assert_eq!(text.len(), 2);
    assert!(
        !text
            .iter()
            .any(|line| line.windows(2).any(|w| w == [b'%' - 32, b's' - 32]))
    );
    let mut state = State::default();
    state.submit(request);
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_EVENT_NOTICE_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let prepared = assets.prepare(&renderer, &state).unwrap();
        crate::capture_test::capture(&renderer, Some(&prepared), scene, &path);
    }
    for _ in 0..1000 {
        state.tick();
        assert!(state.request().is_some());
    }
    assert_eq!(state.tick(), None);
    assert!(
        state.request().is_none(),
        "card timeout carries no Lua dismissal callback"
    );
    let _ = scene;
}

#[cfg(test)]
mod tests {
    fn reminder_level(
        labels: openmt2001::level_catalog::Labels,
        id: i32,
    ) -> openmt2001::level_catalog::Level {
        let mut catalog = openmt2001::level_catalog::Catalog::default();
        catalog
            .register(
                openmt2001::level_catalog::Registration {
                    id,
                    directory: b"synthetic".to_vec(),
                    labels,
                    checksum: 0,
                },
                true,
                true,
            )
            .unwrap();
        catalog.levels.remove(0)
    }
    #[test]
    fn scenario_reminder_uses_active_level_text_and_one_ok_button() {
        use openmt2001::level_catalog::Labels;
        let level = reminder_level(
            Labels::Localized {
                title: 100,
                description: 101,
            },
            200003,
        );
        let request = super::scenario_reminder(&level, false, 6);
        let dialog = request.dialog.unwrap();
        assert_eq!(
            (dialog.title, dialog.body, dialog.portrait),
            (100, 101, 302)
        );
        assert_eq!((dialog.buttons, dialog.button_count), ([1437, 0], 1));
        assert_eq!((dialog.callback, dialog.context), (0, 0));
        let custom = reminder_level(
            Labels::Literal {
                title: b"Custom title".to_vec(),
                description: b"Custom objective".to_vec(),
            },
            500000,
        );
        let custom = super::scenario_reminder(&custom, false, 6);
        let script = custom.script.unwrap();
        assert_eq!(script.title.as_deref(), Some(b"Custom title".as_slice()));
        assert_eq!(script.body.as_deref(), Some(b"Custom objective".as_slice()));
    }
    #[test]
    fn scenario_reminder_skirmish_uses_selected_target() {
        use openmt2001::level_catalog::Labels;
        let mut level = reminder_level(
            Labels::Localized {
                title: 100,
                description: 101,
            },
            300001,
        );
        level.set_objective_target(3500.);
        let dialog = super::scenario_reminder(&level, false, 6).dialog.unwrap();
        assert_eq!((dialog.title, dialog.body), (1826, 101));
        assert_eq!(
            dialog.title_argument,
            Some(crate::human_dialog::Argument::Integer(3500))
        );
    }

    #[test]
    #[ignore = "original prompt complete glyph streams at037f"]
    fn matches_native_prompt_text_geometry() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_PROMPT_TEXT")).unwrap();
        assert_eq!(&bytes[..8], b"MTPGTX01");
        let mut at = 8;
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        assert_eq!(word(&mut at), 0x037f);
        let mut fonts = std::collections::BTreeMap::new();
        for _ in 0..word(&mut at) {
            let key = word(&mut at);
            let length = word(&mut at) as usize;
            fonts.insert(key, &bytes[at..at + length]);
            at += length;
        }
        for case in 0..word(&mut at) {
            let font = word(&mut at);
            let style = word(&mut at);
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let x_margin = word(&mut at) as i32;
            let y_margin = word(&mut at) as i32;
            let parent_offset = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let mut paragraphs = Vec::new();
            for _ in 0..word(&mut at) {
                let length = word(&mut at) as usize;
                paragraphs.push(bytes[at..at + length].to_vec());
                at += length;
            }
            let quads = super::text_quads(
                fonts[&font],
                &paragraphs,
                bounds,
                style == 4,
                x_margin,
                y_margin,
                parent_offset,
            )
            .unwrap();
            for page in 0..2 {
                let count = word(&mut at) as usize;
                let actual = quads.iter().filter(|q| q.page == page).collect::<Vec<_>>();
                assert_eq!(actual.len(), count, "native prompt case{case} page{page}");
                for (glyph, quad) in actual.iter().enumerate() {
                    for corner in 0..4 {
                        for axis in 0..3 {
                            assert_eq!(
                                quad.positions[corner][axis].to_bits(),
                                word(&mut at),
                                "native prompt case{case} glyph{glyph} position{corner}/{axis}"
                            );
                        }
                    }
                }
                for (glyph, quad) in actual.iter().enumerate() {
                    for corner in 0..4 {
                        for axis in 0..2 {
                            assert_eq!(
                                quad.uv[corner][axis].to_bits(),
                                word(&mut at),
                                "native prompt case{case} glyph{glyph} uv{corner}/{axis}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    fn deferred_worker_callback_preserves_full_queue_replacement() {
        let mut state = super::State::default();
        let request = |id| super::Request {
            script: None,
            title: id,
            body: id,
            dismissal_id: id,
            dialog: None,
            card: None,
        };
        for id in 1..=4 {
            state.submit(request(id));
        }
        assert_eq!(state.begin_dismiss(), Some(1));
        assert_eq!(state.request().unwrap().dismissal_id, 1);
        assert_eq!(state.begin_dismiss(), None);
        state.finish_dismiss(vec![request(5)]);
        for id in [2, 3, 5] {
            assert_eq!(state.dismiss_with(|_, _| {}), Some(id));
        }
        assert!(state.request().is_none());
    }
    #[test]
    #[ignore = "private original card notice calls"]
    fn matches_native_card_notice() {
        use openmt2001::event_cards::effects::Notice;
        let executable = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_EVENT_NOTICE")).unwrap();
        assert_eq!(&bytes[..8], b"MTECN002");
        let mut at = 8;
        let read = |at: &mut usize| {
            let word = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            word
        };
        for _ in 0..read(&mut at) {
            let [card, character, gate, affected, amount, count, _] =
                std::array::from_fn(|_| read(&mut at));
            let expected: Vec<[u32; 5]> = (0..count)
                .map(|_| std::array::from_fn(|_| read(&mut at)))
                .collect();
            if gate != 0 {
                assert!(
                    expected.is_empty(),
                    "original current-player/suppression gate"
                );
                continue;
            }
            let mut actor = vec![0; 0x220];
            actor[0x150..0x154].copy_from_slice(&character.to_le_bytes());
            actor[0x158..0x166].copy_from_slice(b"Original Actor");
            let request = super::card_request(
                &image,
                &actor,
                Notice {
                    player: 0,
                    card,
                    enabled: true,
                    affected: affected != 0,
                    amount: f32::from_bits(amount),
                },
            )
            .unwrap();
            let presentation = request.card.unwrap();
            assert_eq!(request.dismissal_id, 0);
            assert_eq!(&presentation.actor[..14], b"Original Actor");
            let mut actual = vec![
                [0x43be20, 0, 0, 0, 0],
                [0x429af0, 1, presentation.portrait, 0, 0],
                [0x429820, 3, 0, request.title, 0],
                [0x441940, 0, 0, 0, 0],
                [0x441150, 2, request.body, 0x10010158, 0],
            ];
            if let Some(amount) = presentation.amount {
                actual.push([0x43a950, 1, amount, 0, 0]);
                actual.push([0x441150, 2, request.body + 1, 0x10040000, 0]);
            }
            actual.extend([[0x429a60, 1, 1, 0, 0], [0x4298b0, 2, 0, 1437, 0]]);
            assert_eq!(actual, expected, "card {card} affected {affected}");
        }
        for _ in 0..read(&mut at) {
            let initial = read(&mut at) as i32;
            let expected_steps = read(&mut at);
            let remaining = read(&mut at) as i32;
            let mut state = super::State::default();
            state.submit(super::Request {
                script: None,
                title: 1,
                body: 2,
                dismissal_id: 3,
                dialog: None,
                card: None,
            });
            state.slots[state.head.unwrap()].remaining = initial;
            for step in 1..=expected_steps {
                let callback = state.tick();
                assert_eq!(callback, (step == expected_steps).then_some(3));
                assert!(state.request().is_some(), "callback runs before unlink");
            }
            assert_eq!(state.slots[state.head.unwrap()].remaining, remaining);
            state.finish_dismiss(Vec::new());
            assert!(state.request().is_none());
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    fn dismissal_callback_runs_before_slot_is_freed() {
        let mut state = super::State::default();
        for id in 1..=4 {
            state.submit(super::Request {
                script: None,
                title: id,
                body: id,
                dismissal_id: id,
                dialog: None,
                card: None,
            });
        }
        assert_eq!(
            state.dismiss_with(|s, id| {
                assert_eq!(id, 1);
                assert_eq!(s.request().unwrap().dismissal_id, 1);
                let replaced = s
                    .submit(super::Request {
                        script: None,
                        title: 5,
                        body: 5,
                        dismissal_id: 5,
                        dialog: None,
                        card: None,
                    })
                    .unwrap();
                assert_eq!(replaced.dismissal_id, 4);
            }),
            Some(1)
        );
        for id in [2, 3, 5] {
            assert_eq!(state.dismiss_with(|_, _| {}), Some(id));
        }
        assert!(state.request().is_none());
    }
    #[test]
    #[ignore = "private original prompt queue fixture"]
    fn matches_native_prompt_queue() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_PROMPT_QUEUE")).unwrap();
        assert_eq!(&bytes[..8], b"MTPQUE01");
        let mut at = 8;
        fn word(b: &[u8], at: &mut usize) -> u32 {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        }
        let index = |i: Option<usize>| i.map_or(u32::MAX, |i| i as u32);
        for _ in 0..word(&bytes, &mut at) {
            let mut state = super::State::default();
            for _ in 0..word(&bytes, &mut at) {
                let command = word(&bytes, &mut at);
                let event = word(&bytes, &mut at);
                let actual = if command != 0 {
                    state.submit(super::Request {
                        script: None,
                        title: command,
                        body: command,
                        dismissal_id: command,
                        dialog: None,
                        card: None,
                    });
                    0
                } else {
                    state.dismiss_with(|_, _| {}).unwrap_or(0)
                };
                assert_eq!(actual, event);
                let mut snapshot = vec![index(state.head), index(state.last)];
                for slot in &state.slots {
                    snapshot.extend([
                        u32::from(slot.active),
                        slot.request.as_ref().map_or(0, |r| r.dismissal_id),
                        index(slot.next),
                    ]);
                }
                for value in snapshot {
                    assert_eq!(value, word(&bytes, &mut at));
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    fn dismissal_requires_matching_release_and_preserves_callback_id() {
        let mut state = super::State::default();
        let bounds = [10., 10., 70., 34.];
        for id in [0, 1, u32::MAX] {
            state.submit(super::Request {
                script: None,
                title: 1898,
                body: 1899,
                dismissal_id: id,
                dialog: None,
                card: None,
            });
            state.cursor([20., 20.]);
            state.left_button(true, bounds);
            state.cursor([0., 0.]);
            assert_eq!(state.left_button(false, bounds), None);
            assert!(state.request().is_some());
            state.cursor([20., 20.]);
            state.left_button(true, bounds);
            state.cancel_press();
            assert_eq!(state.left_button(false, bounds), None);
            assert!(state.request().is_some());
            state.left_button(true, bounds);
            assert_eq!(state.left_button(false, bounds), (id != 0).then_some(id));
            assert!(state.request().is_none());
        }
    }
    #[test]
    #[ignore = "private original prompt wrapping fixture"]
    fn matches_native_prompt_wrap() {
        check_native_prompt_wrap("OPENMT2001_NATIVE_PROMPT_WRAP", 0);
    }
    #[test]
    #[ignore = "private original production prompt font gap−1/037f wrapping fixture"]
    fn matches_native_production_prompt_wrap() {
        check_native_prompt_wrap("OPENMT2001_NATIVE_PRODUCTION_PROMPT_WRAP", -1);
    }
    #[test]
    #[ignore = "private original prompt mesh and native parent input fixture"]
    fn matches_native_prompt_parent_input() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let bytes = read("OPENMT2001_NATIVE_PROMPT_INPUT");
        assert_eq!(&bytes[..8], b"MTPINP01");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x037f);
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find("gamedata2/brand_new_panels/prompt_panel.tm")
            .unwrap();
        let bounds = super::parent_input_bounds(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        for (i, bound) in bounds.iter().enumerate() {
            assert_eq!(
                bound.to_bits(),
                u32::from_le_bytes(bytes[12 + i * 4..16 + i * 4].try_into().unwrap())
            );
        }
        let count = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 36 + count * 24);
        for row in bytes[36..].chunks_exact(24) {
            let word = |at| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
            let offset = [f32::from_bits(word(0)), f32::from_bits(word(4))];
            let visible = word(8) != 0;
            let pointer = [word(12) as i32 as f32, word(16) as i32 as f32];
            assert_eq!(
                visible && super::parent_input_hit(bounds, pointer, offset),
                word(20) != 0,
                "native prompt visible={visible} offset={offset:?} pointer={pointer:?}"
            );
        }
    }
    fn check_native_prompt_wrap(key: &str, gap: i32) {
        let bytes = std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        assert_eq!(&bytes[..8], b"MTPWRP01");
        let mut at = 8;
        fn word(b: &[u8], at: &mut usize) -> usize {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap()) as usize;
            *at += 4;
            v
        }
        let mut fonts = vec![];
        for _ in 0..word(&bytes, &mut at) {
            let n = word(&bytes, &mut at);
            fonts.push(&bytes[at..at + n]);
            at += n;
        }
        for case in 0..word(&bytes, &mut at) {
            let font = word(&bytes, &mut at);
            let id = word(&bytes, &mut at);
            let width = word(&bytes, &mut at);
            let len = word(&bytes, &mut at);
            let count = word(&bytes, &mut at);
            let text = &bytes[at..at + len];
            at += len;
            let expected = (0..=count)
                .map(|_| word(&bytes, &mut at))
                .collect::<Vec<_>>();
            let copy = &bytes[at..at + len];
            at += len;
            let actual = super::wrap_with_gap(fonts[font], text, width as i32, gap).unwrap();
            assert_eq!(actual.0, expected, "case{case} id{id} width{width}");
            assert_eq!(actual.1, copy);
        }
        assert_eq!(at, bytes.len());
    }
}

#[test]
#[ignore = "requires original modal resources and GPU"]
fn original_human_dialog_resources() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe = openmt2001::fixtures::path("EXE");
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let offsets = fs::read(language.join("tycoon.off")).unwrap();
    let bytes = fs::read(language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings = offsets
        .chunks_exact(2)
        .map(|n| {
            let end = at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
            let s = &bytes[at..end];
            at = end;
            s
        })
        .collect::<Vec<_>>();
    let assets = Assets::load(&index, &archive, installer, &strings, &image).unwrap();
    let mut name = [0; 96];
    name[..4].copy_from_slice(b"Dog\0");
    let category = u32::from_le_bytes(image.at(0x5385a0 + 33 * 88, 4).unwrap().try_into().unwrap());
    let portrait = u32::from_le_bytes(image.at(0x541afc + 2 * 40, 4).unwrap().try_into().unwrap());
    let mut dialog = crate::human_dialog::Dialog::default();
    for (address, args, count) in [
        (0x429af0, [portrait, 0, 0, 0], 1),
        (0x429820, [0, 0x6b0, 0, 0], 3),
        (0x429a60, [2, 0, 0, 0], 1),
        (0x4298b0, [0, 0x6a8, 0, 0], 2),
        (0x4298b0, [1, 0x6a9, 0, 0], 2),
        (0x441940, [0; 4], 0),
        (0x440d40, [0x6b1, 0, 1750, category], 4),
        (0x429ac0, [0; 4], 1),
    ] {
        dialog.ui(address, args, count, Some(name)).unwrap();
    }
    dialog.callback = 0x48b350;
    dialog.context = 0x23;
    let text = dialog.paragraphs(&assets.strings).unwrap();
    println!(
        "{}",
        String::from_utf8(text[0].iter().map(|b| b + 32).collect()).unwrap()
    );
    assert_eq!(
        String::from_utf8(text[0].iter().map(|b| b + 32).collect()).unwrap(),
        "Dog has made an offer of 1750 for ATLANTIC AVENUE. What would you like to do?"
    );
    let mut state = State::default();
    state.submit(Request {
        script: None,
        title: 0,
        body: 0,
        dismissal_id: 0,
        card: None,
        dialog: Some(dialog),
    });
    for _ in 0..1200 {
        state.tick();
    }
    assert!(state.request().is_some(), "offer timeout disabled");
    let bounds = assets.dialog_buttons(2);
    state.cursor([
        (bounds[1][0] + bounds[1][2]) * 0.5,
        (bounds[1][1] + bounds[1][3]) * 0.5,
    ]);
    state.dialog_mouse(true, &bounds);
    state.dialog_mouse(false, &bounds);
    assert_eq!(
        state.take_dialog_replies(),
        vec![crate::human_dialog::Reply {
            callback: 0x48b350,
            context: 0x23,
            button: 1
        }]
    );
    assert!(
        state.request().is_some(),
        "slot stays live until synchronous reply finishes"
    );
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_HUMAN_DIALOG_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let panel = assets.prepare(&renderer, &state).unwrap();
        crate::capture_test::capture_standalone(&renderer, &panel, &path);
        let mut tip = State::default();
        tip.submit(Request {
            script: None,
            title: 1934,
            body: 1935,
            dismissal_id: 1,
            dialog: None,
            card: None,
        });
        tip.cursor([-1.; 2]);
        let panel = assets.prepare(&renderer, &tip).unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &panel,
            "work/frontend/popup-starting-out-native-text.png",
        );
        let panel = assets
            .prepare_message_positioned(&renderer, &tip, [0., -140.])
            .unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &panel,
            "work/frontend/popup-positioned-message-native-text.png",
        );
        let mut actor = vec![0; 0x220];
        actor[0x150..0x154].copy_from_slice(&2u32.to_le_bytes());
        actor[0x158..0x15e].copy_from_slice(b"Justin");
        let mut card = State::default();
        card.submit(
            card_request(
                &image,
                &actor,
                openmt2001::event_cards::effects::Notice {
                    player: 0,
                    card: 16,
                    enabled: true,
                    affected: true,
                    amount: -1234.125,
                },
            )
            .unwrap(),
        );
        card.cursor([-1.; 2]);
        let panel = assets.prepare(&renderer, &card).unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &panel,
            "work/frontend/popup-card-native-text.png",
        );
    }
    state.finish_dismiss(Vec::new());
    assert!(state.request().is_none());
}

fn error_glyphs(strings: &[Vec<u8>], message: &str) -> Result<Vec<u8>, String> {
    let map = strings.first().ok_or("Language map missing")?;
    message
        .chars()
        .take(400)
        .map(|c| {
            let b = if c.is_ascii() && !c.is_control() {
                c as u8
            } else {
                b'?'
            };
            map.get((b - 32) as usize)
                .copied()
                .ok_or("Error glyph missing".to_string())
        })
        .collect()
}
#[cfg(test)]
mod route_error_text_tests {
    use super::*;
    #[test]
    fn error_body_is_visible_glyph_data_for_existing_prompt() {
        let rows = vec![(0..224).map(|i| i as u8).collect()];
        assert_eq!(
            error_glyphs(&rows, "Worker failed").unwrap(),
            b"Worker failed".iter().map(|b| b - 32).collect::<Vec<_>>()
        );
        assert_eq!(error_glyphs(&rows, "é").unwrap(), vec![b'?' - 32]);
    }
}

#[cfg(test)]
mod owned_script_tests {
    use super::*;
    fn request(id: u32) -> Request {
        Request {
            title: 0,
            body: 0,
            dismissal_id: id,
            dialog: None,
            card: None,
            script: Some(ScriptText {
                title: Some(format!("Synthetic {id}").into_bytes().into()),
                body: Some(b"A test zone is available.".as_slice().into()),
                format_value: 0,
            }),
        }
    }
    #[test]
    fn byte_mapping_and_safe_format_policy() {
        let row: Vec<u8> = (0..224).map(|i| (i as u8).wrapping_add(7)).collect();
        assert_eq!(
            literal_glyphs(b"A%%\n\xff\0ignored", &row).unwrap(),
            vec![row[33], row[5], row[0x3c], row[0x4e], row[223]]
        );
        for end in [b'\r', b'\t', 1] {
            assert_eq!(
                literal_glyphs(&[b'A', end, b'B'], &row).unwrap(),
                vec![row[33]]
            );
        }
        for raw in [b"%s".as_slice(), b"%d", b"%", b"%08x"] {
            assert!(literal_glyphs(raw, &row).is_err());
        }
    }
    #[test]
    fn four_owned_slots_replacement_and_deferred_callback() {
        let mut state = State::default();
        for id in 1..=4 {
            state.submit(request(id));
        }
        let retained = state.request().unwrap();
        assert_eq!(state.begin_dismiss(), Some(1));
        state.finish_dismiss(vec![request(5)]);
        for id in [2, 3, 5] {
            let r = state.request().unwrap();
            assert_eq!(
                r.script.unwrap().title.unwrap().as_ref(),
                format!("Synthetic {id}").as_bytes()
            );
            assert_eq!(state.dismiss_with(|_, _| {}), Some(id));
        }
        assert_eq!(
            retained.script.unwrap().title.unwrap().as_ref(),
            b"Synthetic 1"
        );
        assert!(state.request().is_none());
    }
    #[test]
    fn render_preparation_bypasses_missing_resource_and_formats_numeric_field() {
        let mut rows = vec![(0..224).map(|i| i as u8).collect::<Vec<_>>()];
        rows.push(b"%d".iter().map(|b| b - 32).collect());
        rows.push(vec![33, 34]);
        let mut r = request(1);
        r.title = u32::MAX;
        assert_eq!(
            script_field(&r, 0, &rows).unwrap().unwrap()[0],
            literal_glyphs(b"Synthetic 1", &rows[0]).unwrap()
        );
        r.body = 1;
        r.script.as_mut().unwrap().body = None;
        r.script.as_mut().unwrap().format_value = 2;
        assert_eq!(script_field(&r, 1, &rows).unwrap(), Some(vec![vec![18]]));
    }
}

#[cfg(test)]
mod resource_integer_tests {
    use super::*;
    fn encoded(raw: &[u8], row: &[u8]) -> Vec<u8> {
        raw.iter().map(|b| row[usize::from(b - 32)]).collect()
    }
    #[test]
    fn signed_decimal_is_not_a_language_reference() {
        for shift in [0u8, 17] {
            let row: Vec<u8> = (0..224).map(|i| (i as u8).wrapping_add(shift)).collect();
            let mut strings = vec![row.clone(); 8];
            strings[1] = encoded(b"Count %d", &row);
            strings[7] = encoded(b"Other", &row);
            let mut r = Request {
                title: 1,
                body: 1,
                dismissal_id: 0,
                dialog: None,
                card: None,
                script: Some(ScriptText {
                    title: None,
                    body: None,
                    format_value: 7,
                }),
            };
            assert_eq!(
                script_field(&r, 0, &strings).unwrap(),
                Some(vec![encoded(b"Count 7", &row)])
            );
            for (word, expected) in [(u32::MAX, b"Count -1".as_slice()), (0, b"Count 0")] {
                r.script.as_mut().unwrap().format_value = word;
                assert_eq!(
                    script_field(&r, 1, &strings).unwrap(),
                    Some(vec![encoded(expected, &row)])
                );
            }
            assert_eq!(
                resource_integer(&encoded(b"%i", &row), &row, u32::MAX).unwrap(),
                encoded(b"-1", &row)
            );
            for raw in [
                b"Count %".as_slice(),
                b"%%",
                b"%%s",
                b"%d %i",
                b"%s",
                b"%07d",
            ] {
                assert!(
                    resource_integer(&encoded(raw, &row), &row, 7).is_err(),
                    "{raw:?}"
                );
            }
        }
    }
    #[test]
    fn actual_synthetic_font_quads_from_prepared_script_fields() {
        let row: Vec<u8> = (0..224).map(|i| i as u8).collect();
        let r = Request {
            title: u32::MAX,
            body: u32::MAX,
            dismissal_id: 0,
            dialog: None,
            card: None,
            script: Some(ScriptText {
                title: Some(b"AB".as_slice().into()),
                body: Some(b"CD".as_slice().into()),
                format_value: 0,
            }),
        };
        let mut tab = vec![0; 4 + 224 * 44];
        tab[..4].copy_from_slice(&224u32.to_le_bytes());
        for glyph in tab[4..].chunks_exact_mut(44) {
            glyph[32..36].copy_from_slice(&8i32.to_le_bytes());
            glyph[36..40].copy_from_slice(&12i32.to_le_bytes());
        }
        // Real field resolution -> wrap -> Font::layout, body first, then centered title.
        let body = text_quads(
            &tab,
            &script_field(&r, 1, &[row.clone()]).unwrap().unwrap(),
            [0., 0., 100., 24.],
            false,
            0,
            0,
            [0., 0.],
        )
        .unwrap();
        let title = text_quads(
            &tab,
            &script_field(&r, 0, &[row]).unwrap().unwrap(),
            [0., 0., 100., 24.],
            true,
            0,
            0,
            [0., 0.],
        )
        .unwrap();
        assert_eq!(body.len(), 2);
        assert_eq!(title.len(), 2);
        assert_eq!(body[0].positions[0][0], 1.5);
        assert!(title[0].positions[0][0] > body[0].positions[0][0]);
        assert_eq!(body[1].positions[0][0] - body[0].positions[0][0], 7.);
    }
}

#[cfg(test)]
mod desktop_quit_assets_tests {
    use super::*;
    #[test]
    #[ignore = "requires supplied v1.4 USA prompt assets; optional capture feature + OPENMT2001_DESKTOP_QUIT_CAPTURE directory"]
    fn original_quit_full_body_colored_buttons_and_offset_surfaces() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        let exe = openmt2001::fixtures::path("EXE");
        let language = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let bytes = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets
            .chunks_exact(2)
            .map(|n| {
                let end = at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
                let row = &bytes[at..end];
                at = end;
                row
            })
            .collect::<Vec<_>>();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let assets = Assets::load(&index, &archive, installer, &strings, &image).unwrap();
        let mut modal = crate::menu::desktop_quit::Modal::default();
        modal.open(None);
        let mut state = State::default();
        state.submit(Request {
            script: None,
            dialog: Some(crate::menu::desktop_quit::dialog()),
            title: 1765,
            body: 1766,
            dismissal_id: 0,
            card: None,
        });
        let surfaces = assets
            .message_surfaces(&state, true, [-40., 0.], Some(&modal))
            .unwrap();
        assert_eq!(
            assets.main.map(f32::to_bits),
            [
                224.8000030517578_f32,
                180.29209899902344,
                505.18377685546875,
                303.6317138671875
            ]
            .map(f32::to_bits)
        );
        assert_eq!(
            assets.input_bounds[0].to_bits(),
            224.00448608398438_f32.to_bits()
        );
        // Shared mesh parts retain their authored positions, shifted once.
        for (base, actual) in assets.base.iter().zip(&surfaces) {
            for (before, after) in base.vertices.iter().zip(&actual.vertices) {
                assert_eq!(after[0], before[0] - 40.);
                assert_eq!(after[1], before[1]);
            }
        }
        let base = assets.base.len();
        let green = &surfaces[base + 1];
        let red = &surfaces[base + 2];
        // No portrait surface between title and two SOLID colored backgrounds.
        assert_eq!([green.width, green.height, red.width, red.height], [1; 4]);
        assert!(
            green
                .vertices
                .iter()
                .any(|v| v[3] == 80. / 255. && v[4] == 148. / 255. && v[5] == 34. / 255.)
        );
        assert!(
            red.vertices
                .iter()
                .any(|v| v[3] == 175. / 255. && v[4] == 6. / 255. && v[5] == 6. / 255.)
        );
        assert!(
            assets
                .desktop_quit_hit_at([250., 274.79681396484375], false)
                .is_some()
        );
        assert_eq!(
            assets.desktop_quit_hit_at([250., 274.79681396484375], true),
            None
        );
        // Exact USA language identity prevents accidentally using the unpatched table.
        let decode = |id: usize| {
            assets.strings[id]
                .iter()
                .map(|b| b + 32)
                .collect::<Vec<_>>()
        };
        assert_eq!(decode(1765), b"QUIT TO DESKTOP");
        assert_eq!(decode(1766), b"Do you want to quit Monopoly Tycoon?");
        assert_eq!(decode(1636), b"Yes");
        assert_eq!(decode(1637), b"No");
        // Body spans the whole default main width, centered with original4/4
        // layout, instead of reserving the portrait-right column.
        let body_bounds = [
            assets.main[0],
            assets.main[1],
            assets.main[2],
            274.79681396484375,
        ];
        let body = text_quads(
            &assets.tabs[0],
            &[assets.strings[1766].clone()],
            body_bounds,
            true,
            4,
            4,
            [-40., 0.],
        )
        .unwrap();
        assert!(!body.is_empty());
        assert!(
            body.iter()
                .flat_map(|q| q.positions)
                .any(|p| p[0] < 261.96099853515625)
        );
        let body_pages = &surfaces[base + 3..base + 3 + assets.fonts[0].len()];
        assert!(
            body_pages
                .iter()
                .flat_map(|s| &s.vertices)
                .any(|v| v[0] < 261.96099853515625)
        );
        // Existing ordinary dialog defaults still include portrait302 and keep
        // the historical portrait-column bounds. This variant is not global.
        let ordinary = assets
            .message_surfaces(&state, true, [0.; 2], None)
            .err()
            .expect("ordinary sentinel must remain unsupported");
        assert!(ordinary.contains("portrait")); // sentinel unsupported outside dedicated variant
        let mut ordinary_state = State::default();
        ordinary_state.submit(Request {
            script: None,
            dialog: Some(crate::human_dialog::Dialog {
                title: 1765,
                body: 1766,
                buttons: [1636, 1637],
                button_count: 2,
                timeout: false,
                ..Default::default()
            }),
            title: 1765,
            body: 1766,
            dismissal_id: 0,
            card: None,
        });
        let ordinary = assets
            .message_surfaces(&ordinary_state, true, [0.; 2], None)
            .unwrap();
        assert_eq!(ordinary[base + 1].vertices, assets.portrait.vertices);
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_DESKTOP_QUIT_CAPTURE") {
            let path = std::path::PathBuf::from(path);
            assert!(path.is_dir(), "supply an owned existing output directory");
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            for (name, point, pressed) in [
                ("idle", None, false),
                ("yes-hover", Some([250., 280.]), false),
                ("yes-pressed", Some([250., 280.]), true),
                ("no-hover", Some([370., 280.]), false),
            ] {
                modal.cancel_pointer();
                if let Some(point) = point {
                    modal.cursor(point);
                }
                if pressed {
                    let hit = assets.desktop_quit_hit(&modal);
                    modal.left(true, hit);
                }
                let panel = assets.prepare_desktop_quit(&renderer, &modal).unwrap();
                let file = path.join(format!("desktop-quit-{name}.png"));
                crate::capture_test::capture_standalone(&renderer, &panel, file.to_str().unwrap());
            }
        }
    }
}
