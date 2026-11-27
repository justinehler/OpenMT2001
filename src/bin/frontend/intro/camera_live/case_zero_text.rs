//! Actual case0 text ownership: 441940,4413b0,441480,440330,440950,
//! 4406a0,4409c0,4e6ac0,4e6b10,4e7a90 and finite4419f0.
//! No English captions or glyph metrics are embedded here.
use openmt2001::{
    font::{Font, GlyphQuad, Layout},
    pe::PeImage,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Group {
    pub bytes: Vec<u8>,
    pub offsets: Vec<u32>,
    pub count: u32,
}
impl Group {
    pub fn clear(&mut self) {
        self.bytes.fill(0);
        self.offsets.fill(0);
        self.count = 0;
    }
    fn reserve_offsets(&mut self, n: usize) {
        if self.offsets.len() < n {
            self.offsets.resize(n, 0)
        }
    }
    fn reserve_bytes(&mut self, n: usize) {
        if self.bytes.len() < n {
            self.bytes.resize(n, 0)
        }
    }
    pub fn row(&self, row: u32) -> Result<&[u8], String> {
        if row >= self.count {
            return Err("text row outside actual group".into());
        }
        let first = *self.offsets.get(row as usize).ok_or("text first offset")? as usize;
        let end = *self
            .offsets
            .get(row as usize + 1)
            .ok_or("text last offset")? as usize;
        let end = end
            .checked_sub(1)
            .ok_or("text delimiter offset underflow")?;
        self.bytes
            .get(first..end)
            .ok_or_else(|| "text row outside actual storage".into())
    }
    /// Native's terminal offset includes one separator, while its byte copy
    /// omits that terminal separator. Capacity tails remain retained storage.
    pub fn append(&mut self, source: &Group) -> Result<(), String> {
        let length = source
            .offsets
            .get(source.count as usize)
            .ok_or("source terminal offset")?
            .wrapping_sub(1);
        self.reserve_offsets(2);
        let prior = *self
            .offsets
            .get(self.count as usize)
            .ok_or("destination terminal offset")?;
        if length == u32::MAX {
            return Ok(());
        } //native failed empty-source append
        let count = self
            .count
            .checked_add(source.count)
            .ok_or("text row count overflow")?;
        self.reserve_offsets(count as usize + 2);
        let end = prior
            .checked_add(length)
            .ok_or("text byte count overflow")?;
        self.reserve_bytes(end as usize);
        self.bytes[prior as usize..end as usize].copy_from_slice(
            source
                .bytes
                .get(..length as usize)
                .ok_or("source byte copy")?,
        );
        for i in 0..source.count {
            self.offsets[self.count as usize] = prior.wrapping_add(source.offsets[i as usize]);
            self.count += 1;
        }
        self.offsets[self.count as usize] =
            prior.wrapping_add(source.offsets[source.count as usize]);
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScratchByte {
    Unwritten,
    Byte(u8),
}
#[derive(Clone, Debug)]
pub struct FontData {
    pub table: Vec<u8>,
    pub baseline: i32,
    pub scale: f32,
}
/// Native manager8b22cc data plus its retained shared mutable state. Bind this
/// one manager to the real UI resource owner; do not recreate it per callback.
#[derive(Clone, Debug)]
pub struct Fonts {
    pub fonts: BTreeMap<u32, FontData>,
    pub language: super::ui_text_resources::PackedLanguage,
    pub current: u32,
    pub gap: i32,
    pub line_mode: u8,
    pub scratch: Vec<ScratchByte>,
    pub formatted: Group,
    pub temporary: Group,
    pub error: ErrorWord,
    pub selector: SelectorState,
    pub context: u32,
    pub glyph_callback: u32,
    pub ready_callback: u32,
    pub four_callback: u32,
    pub pair: [u32; 2],
    pub tail: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorWord {
    Unknown,
    Known(u32),
}
#[derive(Clone, Debug)]
pub struct SelectorState {
    pub cached: u32,
    pub pending: [u32; 2],
    pub notify: u32,
}
impl Fonts {
    pub fn select(&mut self, id: u32) -> Result<(), String> {
        //455420 cache hit returns before any manager/index/batch read.
        if self.selector.cached == id {
            return Ok(());
        }
        if self.selector.pending != [0, 0] {
            return Err("font selector requires unsupported pending page-batch flush".into());
        }
        self.direct_index(id)?;
        self.selector.cached = id;
        Ok(())
    }
    ///455420 finite retained construction path. Cache hit precedes all reads;
    ///missing page-batch/notify callbacks remain unsupported frontiers.
    ///4e7470 stores current BEFORE calling notify; cache publishes afterward.
    pub(super) fn select455420_retained(&mut self, id: u32) -> Result<(), String> {
        if self.selector.cached == id {
            return Ok(());
        }
        if self.selector.pending != [0, 0] {
            return Err("455420 unsupported pending page-batch callback frontier".into());
        }
        if !self.fonts.contains_key(&id) {
            return Err(format!("actual font{id} not registered"));
        }
        self.current = id;
        if self.selector.notify != 0 {
            return Err("4e7470 unsupported notify callback after retained current store".into());
        }
        self.selector.cached = id;
        Ok(())
    }
    ///4e7470; deliberately does NOT update global6aad08 selector cache.
    pub fn direct_index(&mut self, id: u32) -> Result<(), String> {
        if !self.fonts.contains_key(&id) {
            return Err(format!("actual font{id} not registered"));
        }
        if self.selector.notify != 0 {
            return Err("font direct setter requires unsupported notify callback".into());
        }
        self.current = id;
        Ok(())
    }
    ///Only4e6e80's actual consumer clears5b4. Unknown is not success0.
    pub fn take_error(&mut self) -> Result<u32, String> {
        let ErrorWord::Known(value) = self.error else {
            return Err("font manager error5b4 has no actual writer/preimage".into());
        };
        self.error = ErrorWord::Known(0);
        Ok(value)
    }
    ///Finite4e72f0 file-open branch; preserves a retained table on failure.
    ///Malformed modern input is a parser error, not an invented native error5.
    pub fn load_table(&mut self, id: u32, path: &std::path::Path) -> Result<(), String> {
        if id >= 32 {
            return Err("font registration outside32 slots".into());
        }
        //4e7313 publishes the requested slot BEFORE fopen/error1.
        self.current = id;
        let table = std::fs::read(path).map_err(|e| {
            self.error = ErrorWord::Known(1);
            format!("font{id} {}: {e}", path.display())
        })?;
        Font::parse(&table).map_err(|e| format!("font{id} {}: {e}", path.display()))?;
        if self.ready_callback != 0 {
            return Err("font table ready callback remains unsupported".into());
        }
        let prior = self.fonts.get(&id);
        let baseline = prior.map_or(0, |f| f.baseline);
        let scale = prior.map_or(1., |f| f.scale);
        self.fonts.insert(
            id,
            FontData {
                table,
                baseline,
                scale,
            },
        );
        Ok(())
    }
    fn data(&self) -> Result<&FontData, String> {
        self.fonts
            .get(&self.current)
            .ok_or_else(|| "actual active font missing".into())
    }
    pub fn metric(&self, g: u8, at: usize) -> Result<i32, String> {
        let t = &self.data()?.table;
        let at = 4 + usize::from(g) * 44 + at;
        Ok(i32::from_le_bytes(
            t.get(at..at + 4)
                .ok_or("actual font glyph metric")?
                .try_into()
                .unwrap(),
        ))
    }
    pub fn width(&self, text: &[u8]) -> Result<i32, String> {
        text.iter().try_fold(0i32, |sum, &g| {
            Ok(sum.wrapping_add(trunc(
                f64::from(self.metric(g, 32)?.wrapping_add(self.gap))
                    * f64::from(self.data()?.scale),
            )?))
        })
    }
    pub fn height(&self, text: &[u8]) -> Result<i32, String> {
        let mut lower = 999u32;
        let mut upper = 0u32;
        for &g in text {
            let rise = self.metric(g, 40)? as u32;
            lower = lower.min(rise);
            let h = trunc(f64::from(self.metric(g, 36)?) * f64::from(self.data()?.scale))? as u32;
            upper = upper.max(rise.wrapping_add(h));
        }
        Ok(upper.wrapping_sub(lower) as i32)
    }
    /// 4e79b0 is a row metric, distinct from the 4e75f0 bounding span.
    /// Its unsigned maximum starts at zero; the returned word is signed.
    pub fn group_row_height(&self, group: &Group, row: u32) -> Result<i32, String> {
        if group.count == 0 {
            return Ok(0);
        }
        let mut upper = 0u32;
        for &g in group.row(row)? {
            let rise = self.metric(g, 40)? as u32;
            let h = trunc(f64::from(self.metric(g, 36)?) * f64::from(self.data()?.scale))? as u32;
            upper = upper.max(rise.wrapping_add(h));
        }
        Ok(upper as i32)
    }
    /// 4e7690 writes actual manager scratch; newline expands to two glyphs.
    pub fn map(&mut self, text: &[u8]) -> Result<Vec<u8>, String> {
        let mut n = 0;
        let mut output = Vec::new();
        for &ch in text {
            if ch == 0 {
                break;
            }
            let add = if ch == 10 {
                vec![self.code(0x3c)?, self.code(0x4e)?]
            } else if ch < 32 {
                break;
            } else {
                vec![self.code(usize::from(ch) - 32)?]
            };
            for g in add {
                *self
                    .scratch
                    .get_mut(n)
                    .ok_or("actual font map scratch capacity")? = ScratchByte::Byte(g);
                output.push(g);
                n += 1;
            }
        }
        Ok(output)
    }
    fn code(&self, i: usize) -> Result<u8, String> {
        self.language
            .bytes()
            .get(i)
            .copied()
            .ok_or_else(|| "actual character translation table missing".into())
    }
    pub fn packed(&self, id: u32) -> Result<Vec<u8>, String> {
        let text = self.language.row(id)?;
        //These three original case0 identifiers consume no variadic argument.
        //A different supplied table with format tokens requires actual args;
        //never invent absent varargs from a convenient English replacement.
        let prefix = self.code(5)?;
        if text.contains(&prefix) {
            return Err(format!(
                "case0 identifier{id} requires unsupported packed formatting arguments"
            ));
        }
        Ok(text.to_vec())
    }
    /// Finite native4e7a90 emission, including >=width and explicit '<n'.
    /// Text owns glyphs AND offsets. This isn't renderer-side rewrapping.
    fn emit(&mut self, text: &[u8], width: i32) -> Result<(), String> {
        let width = if width == 0 { 999999 } else { width };
        if width < 0 {
            return Err("unsupported negative case0 text width".into());
        }
        let space = self.code(0)?;
        let newline = self.code(0x3c)?;
        let n = self.code(0x4e)?;
        let mut spans = Vec::new();
        let mut scan = 0usize;
        let mut start = 0usize;
        let mut running = 0i32;
        let mut at_break: Option<usize> = None;
        while !text.is_empty() {
            let mut forced = false;
            if scan < text.len() {
                let advance = trunc(
                    f64::from(self.metric(text[scan], 32)?.wrapping_add(self.gap))
                        * f64::from(self.data()?.scale),
                )?;
                running = running.wrapping_add(advance);
                if running < width || at_break.is_none() {
                    scan += 1;
                    if self.line_mode == 0 {
                        at_break = Some(scan)
                    }
                    if text
                        .get(scan)
                        .is_some_and(|&g| i32::from(g) == i32::from(space as i8))
                    {
                        at_break = Some(scan)
                    }
                    if text.get(scan) == Some(&newline) && text.get(scan + 1) == Some(&n) {
                        at_break = Some(scan + 1);
                        forced = true;
                    }
                    if !forced && scan < text.len() {
                        continue;
                    }
                }
            }
            if !forced && running < width {
                spans.push((start, text.len() + 1));
                break;
            }
            if let Some(split) = at_break {
                let next = split + 1;
                spans.push((start, next));
                start = next;
                scan = next;
                running = 0;
                at_break = None;
                if self.line_mode == 0 || scan >= text.len() {
                    break;
                }
            } else {
                spans.push((start, scan + 1));
                if scan >= text.len() {
                    break;
                }
                return Err("unsupported native narrow-width case0 emitter cycle".into());
            }
        }
        let group = &mut self.formatted;
        group.reserve_bytes(text.len());
        group.reserve_offsets(2);
        group.clear();
        group.bytes[..text.len()].copy_from_slice(text);
        for (i, &(first, last)) in spans.iter().enumerate() {
            group.reserve_offsets(i + 2);
            group.offsets[i] = first as u32;
            group.offsets[i + 1] = last as u32;
            group.count += 1;
        }
        for i in 0..text.len().saturating_sub(1) {
            if group.bytes[i] == newline {
                group.bytes[i] = space
            }
        }
        Ok(())
    }
}
pub fn trunc(v: f64) -> Result<i32, String> {
    let v = v.trunc();
    if !v.is_finite() || v < f64::from(i32::MIN) || v > f64::from(i32::MAX) {
        return Err("case0 conversion outside finite native i32 domain".into());
    }
    Ok(v as i32)
}
///Finite signed-result layout admission for50e928. Preserve fractional
///values whose truncation still fits (MIN-1 < x < MAX+1).
pub(super) fn ftol_layout(s: openmt2001::x87::Sum) -> Result<i32, String> {
    use openmt2001::x87::Sum;
    use std::cmp::Ordering;
    let mut lower = Sum::from_i32(i32::MIN);
    lower.add(Sum::from_i32(-1));
    let mut upper = Sum::from_i32(i32::MAX);
    upper.add(Sum::from_i32(1));
    if s.compare(lower) != Ordering::Greater || s.compare(upper) != Ordering::Less {
        return Err("layout FTOL outside admitted signed i32 result domain".into());
    }
    Ok(s.ftol_low_word())
}
/// Actual Text vtable+2c target used by441940. Other subtype targets are
/// retained as unavailable; a direct4406a0 call is a separate operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetTarget {
    Ret43b370,
    Unsupported(u32),
}
impl ResetTarget {
    pub fn from_source_target(target: u32) -> Self {
        if target == 0x43b370 {
            Self::Ret43b370
        } else {
            Self::Unsupported(target)
        }
    }
}
#[derive(Clone, Debug)]
pub struct Text {
    pub reset_target: ResetTarget,
    pub bounds: [f32; 4],
    pub depth: f32,
    pub font: u32,
    pub pitch: i32,
    pub gap: i8,
    pub visible: u32,
    pub line_mode: u8,
    pub scale: f32,
    pub padding: i32,
    pub colors: Vec<u8>,
    pub blink_colors: Vec<u8>,
    pub blink: Vec<u8>,
    pub x: Vec<i16>,
    pub extent: [f32; 4],
    pub group: Group,
    pub y: i16,
    pub inset: i32,
    pub scroll: i32,
    pub style: u32,
    pub first: u32,
    pub dirty: u8,
    pub geometry_dirty: u8,
    pub background_color: u32,
    pub background: u8,
}
impl Text {
    ///440160/440540 successful normal-field result. Old heap pitch and
    ///transient148 are not consumed by a live normal-success field. Native
    ///x87 status/traps and allocation-failure unwind are outside this API.
    ///Node context/padding live in the typed Node's explicit Unknown carrier.
    pub(super) fn normal_fields() -> Self {
        Self {
            reset_target: ResetTarget::Ret43b370,
            bounds: [0.; 4],
            depth: 0.5,
            font: 0,
            pitch: 10,
            gap: 0,
            visible: 64,
            line_mode: 1,
            scale: 1.,
            padding: 0,
            colors: Vec::new(),
            blink_colors: Vec::new(),
            blink: Vec::new(),
            x: Vec::new(),
            extent: [0.; 4],
            group: Group {
                bytes: Vec::new(),
                offsets: Vec::new(),
                count: 0,
            },
            y: 0,
            inset: 0,
            scroll: 0,
            style: 0,
            first: 0,
            dirty: 0,
            geometry_dirty: 0,
            background_color: 0,
            background: 0,
        }
    }
    pub(super) fn extent440900(&mut self, bounds: [f32; 4]) -> Result<(), String> {
        if self.pitch == 0 {
            return Err("440900 Text live pitch zero".into());
        }
        self.bounds = bounds;
        let mut ratio = openmt2001::x87::Sum::from_f32(bounds[3]).map_err(str::to_owned)?;
        ratio.add(
            openmt2001::x87::Sum::from_f32(bounds[1])
                .map_err(str::to_owned)?
                .negated(),
        );
        ratio
            .div(openmt2001::x87::Sum::from_i32(self.pitch))
            .map_err(str::to_owned)?;
        self.visible = ftol_layout(ratio)? as u32;
        self.dirty = 0;
        Ok(()) //native setter ignores incoming depth
    }
    pub(super) fn style440650(&mut self, style: u32, inset: i32, scroll: i32) {
        if inset != -999 {
            self.inset = inset
        }
        if scroll != -999 {
            self.scroll = scroll
        }
        self.style = style;
        self.dirty = 0;
    }
    ///440b70 always reaches455420, even when Text13c already equals id.
    pub fn set_font(&mut self, font: &mut Fonts, id: u32) -> Result<(), String> {
        if self.font != id {
            self.font = id;
            self.dirty = 0;
        }
        font.select(id)
    }
    ///Unpadded441220 through4412ed, before440330's array growth.
    ///One SAME Fonts/Group algorithm for the retained construction callers.
    ///A reached modern refusal retains preceding effects; no cleanup/replay.
    pub(super) fn identifier441220_before_resize(
        &mut self,
        font: &mut Fonts,
        id: u32,
    ) -> Result<(), String> {
        if self.padding != 0 {
            return Err("441220 unpadded construction branch required".into());
        }
        font.select455420_retained(self.font)?;
        font.fonts
            .get_mut(&font.current)
            .ok_or("441220 actual current font absent")?
            .scale = self.scale;
        let saved = font.line_mode;
        font.line_mode = self.line_mode;
        let mut width = openmt2001::x87::Sum::from_f32(self.bounds[2]).map_err(str::to_owned)?;
        width.add(
            openmt2001::x87::Sum::from_f32(self.bounds[0])
                .map_err(str::to_owned)?
                .negated(),
        );
        width.add(openmt2001::x87::Sum::from_i32(self.inset.wrapping_mul(2)).negated());
        let width = ftol_layout(width)?;
        //4e8435 writes error4 for an out-of-range id. The modern safety
        //boundary refuses instead of appending a stale formatter product.
        if let Err(error) = font.language.row(id) {
            font.error = ErrorWord::Known(4);
            return Err(error);
        }
        let glyphs = font.packed(id)?; //actual unsupported varargs never invented
        font.emit(&glyphs, width)?;
        self.group.append(&font.formatted)?;
        self.dirty = 0;
        font.line_mode = saved;
        font.fonts
            .get_mut(&font.current)
            .ok_or("441220 active font disappeared")?
            .scale = 1.;
        Ok(())
    }
    ///Retained construction441220 using the existing ordinary array carrier.
    ///Unwritten native X provenance is a separate owner concern; no alignment.
    pub(super) fn identifier441220_unpadded(
        &mut self,
        font: &mut Fonts,
        id: u32,
    ) -> Result<(), String> {
        self.identifier441220_before_resize(font, id)?;
        self.resize()
    }
    ///440b70 using the retained selector frontier for original constructors.
    pub(super) fn set_font440b70_retained(
        &mut self,
        font: &mut Fonts,
        id: u32,
    ) -> Result<(), String> {
        if self.font != id {
            self.font = id;
            self.dirty = 0;
        }
        font.select455420_retained(id)
    }
    ///Plain441220 with explicit unknown X allocation state, no4406a0.
    ///The retained caller owns this SAME Text and its actual X capacity carrier.
    pub(super) fn identifier441220_unaligned(
        &mut self,
        font: &mut Fonts,
        id: u32,
        x: &mut Vec<Option<i16>>,
    ) -> Result<(), String> {
        if !self.x.is_empty() || x.len() != self.colors.len() {
            return Err("441220 unaligned owner/capacity domain differs".into());
        }
        self.identifier441220_before_resize(font, id)?;
        self.resize_unaligned(x)
    }
    ///One native440330 growth body for retained unaligned Text owners.
    ///Native X storage has capacity but no first writer on construction paths.
    pub(super) fn resize_unaligned(&mut self, x: &mut Vec<Option<i16>>) -> Result<(), String> {
        if !self.x.is_empty() || x.len() != self.colors.len() {
            return Err("unaligned Text capacity authority differs".into());
        }
        let n = self.group.count as usize;
        if n > i16::MAX as usize {
            return Err(
                "440330 unaligned arrays exceed signed16 capacity after append/restoration".into(),
            );
        }
        if self.colors.len() < n {
            self.colors.resize(n, 0);
            self.blink_colors.resize(n, 0);
            self.blink.resize(n, 0);
            x.resize(n, None);
        }
        if self.blink_colors.len() != x.len() || self.blink.len() != x.len() {
            return Err("440330 unaligned capacity owners differ".into());
        }
        Ok(())
    }
    pub(super) fn reset_unaligned(&mut self, x: &mut Vec<Option<i16>>) -> Result<(), String> {
        self.reset_before_resize()?;
        self.resize_unaligned(x)?;
        self.colors[..self.group.count as usize].fill(0);
        Ok(())
    }
    pub fn resize(&mut self) -> Result<(), String> {
        let n = self.group.count as usize;
        if n > i16::MAX as usize {
            return Err("text arrays leave native signed16 capacity domain".into());
        }
        if self.colors.len() < n {
            self.colors.resize(n, 0);
            self.blink_colors.resize(n, 0);
            self.blink.resize(n, 0);
            self.x.resize(n, 0)
        }
        if self.blink_colors.len() < n || self.blink.len() < n || self.x.len() < n {
            return Err("inconsistent actual text array capacities".into());
        }
        Ok(())
    }
    /// 441940: clears buffers, calls actual virtual layout, updates +148,
    /// resizes and clears colors for the actual now-empty group only.
    pub fn reset(&mut self, _font: &mut Fonts) -> Result<(), String> {
        self.reset_before_resize()?;
        self.resize()?;
        for c in &mut self.colors[..self.group.count as usize] {
            *c = 0;
        }
        Ok(())
    }
    fn reset_before_resize(&mut self) -> Result<(), String> {
        self.group.clear();
        if let ResetTarget::Unsupported(target) = self.reset_target {
            return Err(format!(
                "441940 Text virtual+2c target{target:08x} unsupported"
            ));
        }
        self.visible = self.visible_ratio()?;
        Ok(())
    }
    fn visible_ratio(&self) -> Result<u32, String> {
        if self.pitch == 0 {
            return Err("actual case0 text pitch zero".into());
        }
        let ratio = (f64::from(self.bounds[3]) - f64::from(self.bounds[1])) / f64::from(self.pitch);
        //This port admits finite layout results only. f64::max would turn
        //NaN or negative infinity into1 before the checked FTOL boundary.
        if !ratio.is_finite() {
            return Err("non-finite Text visible-row ratio".into());
        }
        Ok(trunc(ratio.max(1.))? as u32)
    }
    fn width(&self) -> Result<i32, String> {
        trunc(
            f64::from(self.bounds[2])
                - f64::from(self.bounds[0])
                - f64::from(self.inset.wrapping_mul(2)),
        )
    }
    pub fn append_glyphs(&mut self, font: &mut Fonts, glyphs: &[u8]) -> Result<(), String> {
        font.select(self.font)?;
        let saved = font.line_mode;
        font.fonts
            .get_mut(&font.current)
            .ok_or("text active font absent")?
            .scale = self.scale;
        font.line_mode = self.line_mode;
        let result = (|| {
            font.emit(glyphs, self.width()?)?;
            self.group.append(&font.formatted)?;
            self.dirty = 0;
            self.resize()
        })();
        font.line_mode = saved;
        font.fonts
            .get_mut(&font.current)
            .ok_or("text active font disappeared")?
            .scale = 1.;
        result
    }
    ///4413b0 alternate list formatter ignores+154 padding.
    pub fn append_packed(&mut self, font: &mut Fonts, id: u32) -> Result<(), String> {
        let glyphs = font.packed(id)?;
        self.append_glyphs(font, &glyphs)
    }
    ///441220 has a distinct nonzero-padding branch through441480 "%u".
    pub fn identifier(&mut self, font: &mut Fonts, id: u32) -> Result<(), String> {
        if self.padding == 0 {
            return self.append_packed(font, id);
        }
        font.select(self.font)?;
        let glyphs = font.packed(id)?;
        font.emit(&glyphs, 0)?;
        let row = font.formatted.row(0)?.to_vec();
        let mut raw = Vec::new();
        for _ in 0..self.padding.max(0) {
            raw.push(b' ')
        }
        for glyph in row {
            raw.push(glyph.wrapping_add(32))
        }
        if raw.len() >= 512 {
            return Err("case0 padded identifier exceeds native literal buffer".into());
        }
        let glyphs = font.map(&raw)?;
        self.append_glyphs(font, &glyphs)
    }
    /// Finite441480 tokens used by5333ac and533278. Actual format bytes come
    /// from supplied PE, and %i consumes the two real integers in order.
    pub fn literal(&mut self, font: &mut Fonts, format: &[u8], args: &[i32]) -> Result<(), String> {
        font.select(self.font)?;
        let raw = literal_bytes(self.padding, format, args)?;
        let glyphs = font.map(&raw)?;
        self.append_glyphs(font, &glyphs)
    }
    ///441770 retained construction branch, zero extra arguments. Uses the
    ///SAME formatter as literal; retains native reached selector/scale/mode.
    pub(super) fn literal441770_unaligned(
        &mut self,
        font: &mut Fonts,
        format: &[u8],
        x: &mut Vec<Option<i16>>,
    ) -> Result<(), String> {
        font.select455420_retained(self.font)?;
        let raw = literal_bytes(self.padding, format, &[])?;
        font.select455420_retained(self.font)?;
        font.fonts
            .get_mut(&font.current)
            .ok_or("441770 active font absent")?
            .scale = self.scale;
        let saved = font.line_mode;
        font.line_mode = self.line_mode;
        let mut width = openmt2001::x87::Sum::from_f32(self.bounds[2]).map_err(str::to_owned)?;
        width.add(
            openmt2001::x87::Sum::from_f32(self.bounds[0])
                .map_err(str::to_owned)?
                .negated(),
        );
        width.add(openmt2001::x87::Sum::from_i32(self.inset.wrapping_mul(2)).negated());
        let width = ftol_layout(width)?;
        let glyphs = font.map(&raw)?;
        font.emit(&glyphs, width)?;
        self.group.append(&font.formatted)?;
        self.dirty = 0;
        font.line_mode = saved;
        font.fonts
            .get_mut(&font.current)
            .ok_or("441770 active font disappeared")?
            .scale = 1.;
        self.resize_unaligned(x)
    }
    pub fn layout(&mut self, font: &mut Fonts) -> Result<(), String> {
        self.resize()?;
        let w = font.map(b"W")?;
        let height = font.height(&w)?;
        self.pitch = height.wrapping_add(i32::from(self.gap));
        let width = f64::from(self.bounds[2]) - f64::from(self.bounds[0]);
        for row in 0..self.group.count {
            let row_width = font.width(self.group.row(row)?)?;
            let pos = match self.style {
                0 | 3 | 6 => (self.inset as u16).wrapping_add(2),
                1 | 4 | 7 | 9 => {
                    trunc((width - f64::from(row_width) + f64::from(self.inset)) * 0.5 + 0.5)?
                        as u16
                }
                2 | 5 | 8 => {
                    trunc(width - f64::from(row_width) - f64::from(self.inset) + 0.5)? as u16
                }
                _ => self.x[row as usize] as u16,
            };
            self.x[row as usize] =
                pos.wrapping_add(trunc(f64::from(self.bounds[0]))? as u16) as i16;
        }
        let count = self.group.count as i32;
        let available = f64::from(self.bounds[3]) - f64::from(self.bounds[1]);
        let text_height = |count: i32, h: i32| {
            count
                .wrapping_mul(h)
                .wrapping_add(count.wrapping_sub(1).wrapping_mul(i32::from(self.gap)))
        };
        let y = match self.style {
            0..=2 => 0,
            3..=5 => trunc(
                (available - f64::from(text_height(if count == 0 { 1 } else { count }, height)))
                    * 0.5,
            )? as u16,
            6..=8 => trunc(available - f64::from(text_height(count, height)))? as u16,
            9 => {
                let h = font.group_row_height(&self.group, 0)?;
                trunc(
                    (available - f64::from(text_height(if count == 0 { 1 } else { count }, h)))
                        * 0.5,
                )? as u16
            }
            _ => self.y as u16,
        };
        self.y = y.wrapping_add(trunc(f64::from(self.scroll) + f64::from(self.bounds[1]))? as u16)
            as i16;
        self.dirty = 1;
        self.geometry_dirty = 1;
        Ok(())
    }
    pub fn measure(&mut self, font: &mut Fonts) -> Result<(), String> {
        font.select(self.font)?;
        if self.dirty == 0 {
            self.layout(font)?;
        }
        let h = self.group.count.wrapping_mul(self.pitch as u32);
        self.geometry_dirty = 0;
        self.extent = [
            0.,
            self.bounds[1],
            0.,
            (f64::from(h) + f64::from(self.bounds[1])) as f32,
        ];
        for row in 0..self.group.count {
            let w = font.width(self.group.row(row)?)?;
            if f64::from(w) > f64::from(self.extent[2]) {
                self.extent[2] = w as f32
            }
        }
        Ok(())
    }
    pub fn clear_view(&mut self, font: &mut Fonts) -> Result<(), String> {
        self.geometry_dirty = 1;
        self.visible = self.visible_ratio()?;
        self.measure(font)?;
        self.extent[3] = (f64::from((self.visible as i32).wrapping_mul(self.pitch))
            + f64::from(self.bounds[1])) as f32;
        Ok(())
    }
    /// 441900 ignores incoming, uses actual global pointerY, includes scroll.
    pub fn row_at(&self, pointer_y: i32) -> Result<u32, String> {
        if self.pitch == 0 {
            return Err("text row-at pitch zero".into());
        }
        Ok(trunc(
            (f64::from(pointer_y.wrapping_sub(3)) - f64::from(self.bounds[1]))
                / f64::from(self.pitch)
                + f64::from(self.first),
        )? as u32)
    }
}
///Common finite literal bytes; no independent Chat formatter.
fn literal_bytes(padding: i32, format: &[u8], args: &[i32]) -> Result<Vec<u8>, String> {
    let mut raw = Vec::new();
    for _ in 0..padding.max(0) {
        raw.push(b' ')
    }
    let mut at = 0;
    let mut arg = 0;
    while at < format.len() && format[at] != 0 {
        let ch = format[at];
        at += 1;
        if ch != b'%' {
            raw.push(ch);
            continue;
        }
        let token = *format.get(at).ok_or("unterminated native format token")?;
        match token {
            b'%' => {
                raw.push(b'%');
                at += 1;
            }
            b'i' | b'd' => {
                let n = args.get(arg).ok_or("case0 native format argument absent")?;
                arg += 1;
                raw.extend_from_slice(n.to_string().as_bytes());
                at += 1;
            }
            _ => {} //native revisits unknown selector as a literal
        }
    }
    if raw.len() >= 512 {
        return Err("case0 literal exceeds native output buffer".into());
    }
    Ok(raw)
}
pub struct GlyphPacket {
    pub font: u32,
    pub color: u32,
    pub glyph: GlyphQuad,
}
pub enum Packet {
    Background {
        bounds: [f32; 4],
        depth: f32,
        color: u32,
    },
    Glyph(GlyphPacket),
}
impl Text {
    /// Actual4419f0 body projection. Root submits these in this order and uses
    /// its actual texture pair per font. No default style/palette reconstruction.
    pub fn paint(
        &mut self,
        font: &mut Fonts,
        placement: [f32; 2],
        tick: u32,
        palette: &[u32],
    ) -> Result<Vec<Packet>, String> {
        font.select(self.font)?;
        font.fonts
            .get_mut(&font.current)
            .ok_or("paint active font absent")?
            .scale = self.scale;
        let result = (|| {
            if self.dirty == 0 {
                self.layout(font)?;
            }
            if self.geometry_dirty != 0 {
                self.measure(font)?;
            }
            let mut out = Vec::new();
            if self.background == 1 {
                out.push(Packet::Background {
                    bounds: [
                        (f64::from(placement[0]) + f64::from(self.bounds[0])) as f32,
                        (f64::from(placement[1]) + f64::from(self.bounds[1])) as f32,
                        (f64::from(placement[0]) + f64::from(self.bounds[2])) as f32,
                        (f64::from(placement[1]) + f64::from(self.bounds[3])) as f32,
                    ],
                    depth: self.depth,
                    color: self.background_color,
                });
            }
            if self.first > self.group.count {
                self.first = 0;
            }
            let rows = (self.group.count - self.first).min(self.visible);
            for row in 0..rows {
                let index = self.first + row;
                let colors = if *self.blink.get(row as usize).ok_or("text blink row")? == 1
                    && (tick & 31) > 16
                {
                    &self.blink_colors
                } else {
                    &self.colors
                };
                let color = *palette
                    .get(*colors.get(index as usize).ok_or("text palette index row")? as usize)
                    .ok_or("text row color outside actual palette")?;
                let x = self.x[index as usize] as i32;
                let x = x.wrapping_add(trunc(f64::from(placement[0]))?);
                let y = (self.pitch as u32)
                    .wrapping_mul(row)
                    .wrapping_add(self.y as i32 as u32)
                    .wrapping_add(trunc(f64::from(placement[1]))? as u32);
                let row_glyphs = self.group.row(index)?;
                if !row_glyphs.is_empty() && font.glyph_callback != 0x4554e0 {
                    return Err("text paint requires unsupported glyph callback identity".into());
                }
                let data = font.data()?;
                let parsed = Font::parse(&data.table)?;
                for glyph in parsed.layout(
                    row_glyphs,
                    Layout {
                        origin: [x as f32, y as f32],
                        depth: self.depth,
                        scale: data.scale,
                        gap: font.gap,
                        baseline: data.baseline,
                    },
                )? {
                    out.push(Packet::Glyph(GlyphPacket {
                        font: font.current,
                        color,
                        glyph,
                    }));
                }
            }
            Ok(out)
        })();
        font.fonts
            .get_mut(&font.current)
            .ok_or("paint active font disappeared")?
            .scale = 1.;
        result
    }
}
#[derive(Clone, Debug)]
pub struct Literals {
    pub version: Vec<u8>,
    pub space: Vec<u8>,
    pub colors: [u32; 4],
}
impl Literals {
    pub fn from_pe(pe: &PeImage<'_>) -> Result<Self, String> {
        fn string(pe: &PeImage<'_>, at: u32, max: usize) -> Result<Vec<u8>, String> {
            let b = pe.at(at, max)?;
            let end = b
                .iter()
                .position(|&b| b == 0)
                .ok_or("case0 PE string unterminated")?;
            Ok(b[..end].to_vec())
        }
        let b = pe.at(0x534ce8, 16)?;
        Ok(Self {
            version: string(pe, 0x5333ac, 64)?,
            space: string(pe, 0x533278, 8)?,
            colors: std::array::from_fn(|i| {
                u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
            }),
        })
    }
}

#[cfg(test)]
mod style_nine_regressions {
    use super::super::main_panel::regression_tests::{fonts, set_metric, text};
    use super::*;
    #[test]
    fn style_nine_centers_from_first_row_rise_plus_truncated_height_only() {
        let mut fonts = fonts();
        fonts.fonts.get_mut(&0).unwrap().scale = 0.5;
        set_metric(&mut fonts, 1, 40, 7);
        set_metric(&mut fonts, 1, 36, 5);
        set_metric(&mut fonts, 2, 36, 100);
        let mut text = text();
        text.style = 9;
        text.gap = 3;
        text.group = Group {
            bytes: vec![1, 0, 2],
            offsets: vec![0, 2, 4],
            count: 2,
        };
        assert_eq!(
            fonts.height(&[1]).unwrap(),
            2,
            "4e75f0 bounding span stays separate"
        );
        assert_eq!(
            fonts.group_row_height(&text.group, 0).unwrap(),
            9,
            "7 + trunc(5*0.5)"
        );
        text.layout(&mut fonts).unwrap();
        assert_eq!(text.y, 39, "trunc((100-(2*9+3))/2)");
        set_metric(&mut fonts, 2, 36, 200);
        text.layout(&mut fonts).unwrap();
        assert_eq!(text.y, 39, "later row cannot move the first-row metric");
    }
    #[test]
    fn group_metric_uses_unsigned_wrapping_maximum_then_returns_signed_word() {
        let mut fonts = fonts();
        set_metric(&mut fonts, 1, 40, -2);
        set_metric(&mut fonts, 1, 36, 1);
        set_metric(&mut fonts, 2, 40, 7);
        set_metric(&mut fonts, 2, 36, 4);
        let group = Group {
            bytes: vec![1, 2],
            offsets: vec![0, 3],
            count: 1,
        };
        assert_eq!(
            fonts.group_row_height(&group, 0).unwrap(),
            -1,
            "unsigned ffffffff wins over 11"
        );
        set_metric(&mut fonts, 1, 36, 4);
        assert_eq!(
            fonts.group_row_height(&group, 0).unwrap(),
            11,
            "wrapping -2+4 becomes 2"
        );
    }
    #[test]
    fn empty_group_and_empty_first_row_return_zero_without_scanning_other_rows() {
        let mut fonts = fonts();
        let mut text = text();
        text.style = 9;
        text.gap = 3;
        let retained_y = text.y;
        text.reset(&mut fonts).unwrap();
        assert_eq!(text.y, retained_y, "default reset virtual target is RET");
        text.layout(&mut fonts).unwrap();
        assert_eq!(text.y, 50, "explicit empty style9 layout centers height0");
        assert_eq!(
            fonts
                .group_row_height(
                    &Group {
                        bytes: Vec::new(),
                        offsets: Vec::new(),
                        count: 0
                    },
                    999
                )
                .unwrap(),
            0
        );
        text.group = Group {
            bytes: vec![0, 2],
            offsets: vec![0, 1, 3],
            count: 2,
        };
        assert_eq!(fonts.group_row_height(&text.group, 0).unwrap(), 0);
        text.layout(&mut fonts).unwrap();
        assert_eq!(text.y, 48, "trunc((100-gap3)/2)");
    }
}

#[cfg(test)]
mod active_font_regressions {
    use super::super::main_panel::regression_tests::{fonts, text};
    use super::*;
    fn divergent() -> Fonts {
        let mut font = fonts();
        let mut active = font.fonts[&0].clone();
        for glyph in 0..256 {
            let at = 4 + glyph * 44;
            active.table[at + 32..at + 36].copy_from_slice(&3i32.to_le_bytes());
            active.table[at + 36..at + 40].copy_from_slice(&7i32.to_le_bytes());
        }
        active.scale = 0.5;
        font.fonts.insert(1, active);
        font.fonts.get_mut(&0).unwrap().scale = 0.75;
        font.select(0).unwrap();
        font.direct_index(1).unwrap();
        assert_eq!((font.selector.cached, font.current), (0, 1));
        font
    }
    #[test]
    fn cache_hit_append_scales_active_slot_wraps_and_resets_that_slot() {
        let mut font = divergent();
        let mut t = text();
        t.scale = 2.;
        t.bounds[2] = 15.;
        //Active glyph width3*scale2: [glyph,space,glyph] wraps at15.
        //Without the active scale it is width9 and remains one row.
        t.append_glyphs(&mut font, &[1, 0, 1]).unwrap();
        assert_eq!(t.group.count, 2);
        assert_eq!(t.group.row(0).unwrap(), [1]);
        assert_eq!(t.group.row(1).unwrap(), [1]);
        assert_eq!((font.selector.cached, font.current), (0, 1));
        assert_eq!(font.fonts[&0].scale, 0.75);
        assert_eq!(font.fonts[&1].scale, 1.);
    }
    #[test]
    fn cache_hit_append_error_restores_active_scale_and_line_mode() {
        let mut font = divergent();
        font.line_mode = 0;
        let mut t = text();
        t.scale = 2.;
        t.bounds[2] = -1.;
        assert!(
            t.append_glyphs(&mut font, &[1])
                .unwrap_err()
                .contains("negative")
        );
        assert_eq!(font.line_mode, 0);
        assert_eq!(t.group.count, 0);
        assert_eq!(font.fonts[&0].scale, 0.75);
        assert_eq!(font.fonts[&1].scale, 1.);
    }
    #[test]
    fn cache_hit_paint_uses_active_table_scale_and_both_page_labels() {
        let mut font = divergent();
        let mut t = text();
        t.scale = 2.;
        t.group = Group {
            bytes: vec![1, 96],
            offsets: vec![0, 3],
            count: 1,
        };
        t.dirty = 1;
        t.geometry_dirty = 0;
        let packets = t.paint(&mut font, [0., 0.], 0, &[0xff123456; 10]).unwrap();
        let glyphs = packets
            .iter()
            .filter_map(|p| {
                if let Packet::Glyph(g) = p {
                    Some(g)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(glyphs.len(), 2);
        assert_eq!(
            glyphs
                .iter()
                .map(|g| (g.font, g.glyph.page))
                .collect::<Vec<_>>(),
            [(1, 0), (1, 1)]
        );
        assert_eq!(
            glyphs[0].glyph.positions,
            [
                [-0.5, 20.5, 0.5],
                [5.5, 20.5, 0.5],
                [5.5, 6.5, 0.5],
                [-0.5, 6.5, 0.5]
            ]
        );
        assert_eq!(glyphs[1].glyph.positions[0], [5.5, 20.5, 0.5]);
        assert_eq!(font.fonts[&0].scale, 0.75);
        assert_eq!(font.fonts[&1].scale, 1.);
    }
    #[test]
    fn cache_hit_paint_error_still_resets_active_slot() {
        let mut font = divergent();
        font.glyph_callback = 0;
        let mut t = text();
        t.scale = 2.;
        t.group = Group {
            bytes: vec![1],
            offsets: vec![0, 2],
            count: 1,
        };
        t.dirty = 1;
        t.geometry_dirty = 0;
        assert!(
            t.paint(&mut font, [0., 0.], 0, &[0; 10])
                .err()
                .expect("unsupported callback must fail")
                .contains("callback")
        );
        assert_eq!(font.fonts[&0].scale, 0.75);
        assert_eq!(font.fonts[&1].scale, 1.);
    }
    #[test]
    fn failed_table_open_changes_current_before_error_and_preserves_cache_and_table() {
        let mut font = divergent();
        font.direct_index(0).unwrap();
        let prior = font.fonts[&1].table.clone();
        let missing = std::env::temp_dir().join(format!(
            "openmt2001-uncreated-font-{}.tab",
            std::process::id()
        ));
        assert!(!missing.exists());
        assert!(font.load_table(1, &missing).is_err());
        assert_eq!((font.current, font.selector.cached), (1, 0));
        assert_eq!(font.fonts[&1].table, prior);
        assert_eq!(font.take_error().unwrap(), 1);
        assert_eq!(font.take_error().unwrap(), 0);
        assert!(font.load_table(32, &missing).is_err());
        assert_eq!(font.current, 1);
    }
}
