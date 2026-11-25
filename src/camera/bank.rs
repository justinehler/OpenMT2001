//! Process-lifetime raw camera storage. Both records retain all 196 bytes,
//! including unknown fields and modes unsupported by numeric camera cores.
//! Callers own UI effects, mode dispatch, input, common processing and sampling.
use crate::x87::Sum;

mod selected_actor;
pub use selected_actor::SelectedActorInput;

pub const RECORD_BYTES: usize = 196;
pub type RawRecord = [u8; RECORD_BYTES];

/// A by-value serialization cut, independent of later owner mutations.
/// The external kind-dirty byte is deliberately absent from these save records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capture {
    records: [RawRecord; 2],
}

impl Capture {
    pub const fn from_records(records: [RawRecord; 2]) -> Self {
        Self { records }
    }
    pub const fn records(&self) -> &[RawRecord; 2] {
        &self.records
    }
    pub const fn into_records(self) -> [RawRecord; 2] {
        self.records
    }
}

/// The single raw authority. Move it through presentation refreshes rather than
/// reconstructing it from a render sample or stale imported save material.
/// No Default: loader storage is only an initial process-allocation operation.
#[derive(Debug, PartialEq, Eq)]
pub struct Bank {
    records: [RawRecord; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrategicTransition {
    Entered,
    Returned,
}

/// Completed initialization stores outside both records. The caller applies
/// these to its separate process storage at the initialization call boundary.
/// These words are not camera save fields or a complete callback registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitializationEffects {
    /// Original 0093b028..0093b088, before callback registration.
    pub cleared_callback_words: [u32; 24],
    /// Original 0093b234, binary32 result of 8 / signed width.
    pub scale_8_over_width: u32,
    /// Original 0093b024, binary32 result of 80 / signed width.
    pub scale_80_over_width: u32,
}

impl Bank {
    pub const fn from_capture(capture: Capture) -> Self {
        Self {
            records: capture.records,
        }
    }
    /// PE loader zero-fill only. Startup/menu producers still precede a fresh
    /// city; never use this as a universal new-city or warm-reset template.
    pub const fn loader_zero_storage() -> Self {
        Self {
            records: [[0; RECORD_BYTES]; 2],
        }
    }
    pub const fn primary(&self) -> &RawRecord {
        &self.records[0]
    }
    pub const fn secondary(&self) -> &RawRecord {
        &self.records[1]
    }
    pub fn primary_kind(&self) -> u32 {
        word(self.primary(), 0)
    }
    pub const fn capture(&self) -> Capture {
        Capture {
            records: self.records,
        }
    }
    /// Full restore is pure byte transport, with no mode validation, fixup or
    /// inferred saved value for the separate kind-dirty byte at 0093b164.
    pub fn restore(&mut self, capture: Capture) {
        self.records = capture.records;
    }
    /// Independent selected-tag restore; excludes all other process storage.
    pub fn restore_primary(&mut self, raw: RawRecord) {
        self.records[0] = raw;
    }
    pub fn restore_secondary(&mut self, raw: RawRecord) {
        self.records[1] = raw;
    }
    /// Transactional producer seam for numeric callbacks/common/sample or later
    /// mode implementations. Start each operation from this authority and commit
    /// its result here; a decoded numeric owner must remain temporary. Failed
    /// operations leave both records intact. External closure effects are not
    /// rolled back. There is no automatic decode or finite-mode requirement.
    pub fn try_update_primary<T, E>(
        &mut self,
        update: impl FnOnce(&mut RawRecord) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut next = self.records[0];
        let result = update(&mut next)?;
        self.records[0] = next;
        Ok(result)
    }
    /// 004e06c0. Raw kind transport plus the exact up/threshold stores.
    /// The caller owns the external byte; it is never serialized in Capture.
    pub fn set_kind(&mut self, kind: u32, external_kind_dirty: &mut u8) {
        *external_kind_dirty = 1;
        let raw = &mut self.records[0];
        put(raw, 0, kind);
        triple(raw, 0x64, [0, 0x3f800000, 0]);
        put(raw, 0x78, 0);
    }
    /// 004dfca0. Incoming coordinates are raw MOV words, even for nonfinite
    /// values. Current/sample/cache/radius/center and secondary remain intact.
    pub fn set_target_words(&mut self, eye: [u32; 3]) {
        let raw = &mut self.records[0];
        raw[0x70..0x73].copy_from_slice(&[1, 1, 0]);
        put(raw, 0x74, 10);
        triple(raw, 0x34, eye);
        triple(raw, 0x40, [0; 3]);
        triple(raw, 0x7c, [0x3fc90fdb, 0, 0]);
    }
    /// 004e0260, finite Z arithmetic at PC64 then binary32 store. X/Y are
    /// raw MOV words. Clears smoothing but does not snap current or sampled.
    /// Nonfinite Z requires an FPU exception/NaN contract outside this finite
    /// arithmetic implementation; rejection is atomic, not a save restriction.
    pub fn set_immediate_words(&mut self, eye: [u32; 3]) -> Result<(), &'static str> {
        let mut z = Sum::from_f32(f32::from_bits(eye[2]))?;
        z.add(Sum::from_f32(f32::from_bits(0x3dcccccd))?.negated());
        let raw = &mut self.records[0];
        raw[0x70..0x73].fill(0);
        put(raw, 0x74, 10);
        triple(raw, 0x34, [eye[0], eye[1], z.to_f32().to_bits()]);
        triple(raw, 0x40, [eye[0], 0, eye[2]]);
        Ok(())
    }
    /// Completed 004e0700 field transformation. Width is the signed word read
    /// by FILD; callers pass the actual configuration parameter (not a policy
    /// default). Ordinary startup uses width 50 and parameter 0 or 2.
    /// Preserves fields outside the initialization mask, including current,
    /// sample, opaque bytes, caches, radius, center and the entire secondary.
    /// Zero-width FPU behavior is outside the finite arithmetic contract; errors
    /// leave the bank and external kind-dirty byte unchanged.
    pub fn initialize_process(
        &mut self,
        width: i32,
        parameter: u32,
        external_kind_dirty: &mut u8,
    ) -> Result<InitializationEffects, &'static str> {
        let divide = |numerator: i32| -> Result<u32, &'static str> {
            let mut value = Sum::from_i32(numerator);
            value.div(Sum::from_i32(width))?;
            Ok(value.to_f32().to_bits())
        };
        let scale = divide(10000)?;
        let scale_8_over_width = divide(8)?;
        let scale_80_over_width = divide(80)?;
        put(&mut self.records[0], 0xa8, parameter);
        self.set_kind(0, external_kind_dirty);
        self.set_target_words([0, 0x40c00000, 0xc1200000]);
        let raw = &mut self.records[0];
        put(raw, 0x9c, width as u32);
        put(raw, 0xa0, 0x42c80000);
        put(raw, 0xa4, scale);
        *external_kind_dirty = 0;
        Ok(InitializationEffects {
            cleared_callback_words: [0; 24],
            scale_8_over_width,
            scale_80_over_width,
        })
    }
    /// 00483e10 only. Does not repair caches or reconstruct an orbit.
    pub fn reset_yaw(&mut self) {
        put(&mut self.records[0], 0x7c, 0xbfc90fdb);
    }
    /// Final 004b9d40 camera stores after its earlier yaw helper and external
    /// reset operations. Does not perform those intervening service/UI calls.
    /// Call reset_yaw at its real earlier boundary, then this at the final
    /// setter boundary; neither is a complete fresh-frame constructor.
    pub fn set_world_reset_final_fields(&mut self, external_kind_dirty: &mut u8) {
        self.set_kind(6, external_kind_dirty);
        let raw = &mut self.records[0];
        triple(raw, 0x34, [0x41d00000, 0x44128000, 0xc42dc000]);
        triple(raw, 0x40, [0x41d00000, 0x43e78000, 0xc414c000]);
    }
    /// Isolated 004baf10 transaction at the raw-kind read AFTER the caller's
    /// primary UI-clear callback. Other UI effects and selection restoration
    /// remain the caller's responsibility. No tick, common, sample or trig runs.
    /// A raw-kind4 input returns even when secondary contains an unknown mode.
    pub fn strategic_after_ui_clear(
        &mut self,
        external_kind_dirty: &mut u8,
    ) -> StrategicTransition {
        if self.primary_kind() == 4 {
            self.records[0] = self.records[1];
            StrategicTransition::Returned
        } else {
            self.records[1] = self.records[0];
            // UI mode3's ordered camera setters, 004b5509 onward.
            self.set_immediate_words([0xc35c0000, 0x44834000, 0xc1f00000])
                .expect("strategic Z is finite");
            self.set_kind(4, external_kind_dirty);
            StrategicTransition::Entered
        }
    }
}

fn word(raw: &RawRecord, at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}
fn put(raw: &mut RawRecord, at: usize, value: u32) {
    raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn triple(raw: &mut RawRecord, at: usize, values: [u32; 3]) {
    for (i, value) in values.into_iter().enumerate() {
        put(raw, at + i * 4, value);
    }
}

#[cfg(test)]
mod tests;
