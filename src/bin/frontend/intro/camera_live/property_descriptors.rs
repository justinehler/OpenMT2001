//! Finite44faf0 pavement descriptor owner and4dfba0 lookup.
//! No renderer/picking/F transaction.
use super::ui_selection::{Selected, SelectionKey, SelectionState};
use crate::selection::pavement_resource::{ReferenceRole, ResolvedBinding};
use openmt2001::{camera::primary::BlockInitialization, level_startup::World, pe::PeImage};
use std::sync::atomic::{AtomicU64, Ordering};

type Result<T> = std::result::Result<T, String>;
const WIDTH: usize = 284;
const PAVEMENT_REFERENCE: u32 = 0x688b48; // native static ROLE, never a local handle/pointer
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn identity() -> Result<u64> {
    identity_from(&NEXT_ID)
}
fn identity_from(next: &AtomicU64) -> Result<u64> {
    next.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        .map_err(|_| "modern descriptor identity space exhausted".into())
}
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// A bit is a value only when its corresponding known bit is set.
/// Zero in an UNKNOWN bit is storage for a symbolic value, never heap evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits {
    pub value: u8,
    pub known: u8,
}
impl Bits {
    const UNKNOWN: Self = Self { value: 0, known: 0 };
    fn exact(value: u8) -> Self {
        Self { value, known: 0xff }
    }
    fn and_or(self, keep: u8, set: u8) -> Self {
        Self {
            value: ((self.value & keep) | set),
            known: (self.known & keep) | !keep | set,
        }
    }
    pub fn exact_value(self) -> Option<u8> {
        (self.known == 0xff).then_some(self.value)
    }
    pub fn bit(self, mask: u8) -> Option<bool> {
        (self.known & mask == mask).then_some(self.value & mask == mask)
    }
}

/// Source-written field representation, deliberately NOT repr(C)/native bytes.
/// Unconsumed unknowns survive constructor/reset/model-copy/append.
#[derive(Debug)]
pub struct Descriptor {
    bytes: [Bits; WIDTH],
}
impl Descriptor {
    fn construct(transform: &ResetTransform) -> Self {
        let mut value = Self {
            bytes: [Bits::UNKNOWN; WIDTH],
        };
        value.reset(transform); //4df9b0 ->4df9c0; no preceding memset
        value
    }
    fn put(&mut self, at: usize, bytes: &[u8]) {
        for (target, &value) in self.bytes[at..at + bytes.len()].iter_mut().zip(bytes) {
            *target = Bits::exact(value);
        }
    }
    fn put_word(&mut self, at: usize, value: u32) {
        self.put(at, &value.to_le_bytes());
    }
    fn put_half(&mut self, at: usize, value: u16) {
        self.put(at, &value.to_le_bytes());
    }
    fn reset(&mut self, transform: &ResetTransform) {
        for at in [0x1c, 0x20, 0x24] {
            self.put_word(at, 0x3f800000);
        }
        let flags = self.bytes[0xb8].and_or(0xfd, 0x0d);
        for at in [0x10, 0x14, 0x18, 4, 8, 0xc] {
            self.put_word(at, 0);
        }
        self.put_half(0x2c, 0);
        self.put_half(0x2e, 0xffff);
        self.put_word(0x28, 0xffffffff);
        self.put_word(0, 0);
        self.put_word(0x34, 0);
        self.put_half(0x30, 0);
        self.put_word(0x38, 0);
        self.put_word(0x3c, 0xffffffff);
        self.put_word(0x40, 0xffffffff);
        self.bytes[0xb8] = flags;
        for (i, &value) in transform.0.iter().enumerate() {
            self.put_word(0xbc + i * 4, value);
        }
        for (i, &value) in transform.0.iter().enumerate() {
            self.put_word(0xec + i * 4, value);
        }
    }
    fn set_model(&mut self, definition: &[u8; 116]) {
        //4dfa40 stores the static reference ROLE then copies all29 definition words.
        self.put_word(0, PAVEMENT_REFERENCE);
        for i in 0..29 {
            self.put_word(0x44 + i * 4, word(definition, i * 4));
        }
    }
    pub fn byte(&self, at: usize) -> Option<Bits> {
        self.bytes.get(at).copied()
    }
    pub fn word(&self, at: usize) -> Result<u32> {
        let bytes = self
            .bytes
            .get(at..at + 4)
            .ok_or("descriptor word outside284 bytes")?;
        let mut result = [0; 4];
        for (out, &value) in result.iter_mut().zip(bytes) {
            *out = value
                .exact_value()
                .ok_or("descriptor read reaches an unwritten native bit")?;
        }
        Ok(u32::from_le_bytes(result))
    }
    pub fn kind(&self) -> Result<i16> {
        let lo = self.bytes[0x30]
            .exact_value()
            .ok_or("unknown descriptor kind")?;
        let hi = self.bytes[0x31]
            .exact_value()
            .ok_or("unknown descriptor kind")?;
        Ok(i16::from_le_bytes([lo, hi]))
    }
    pub fn translation(&self) -> Result<[u32; 3]> {
        Ok([self.word(4)?, self.word(8)?, self.word(0xc)?])
    }
}

/// Exact supplied00559730 table, not a newly invented matrix/default.
pub struct ResetTransform([u32; 12]);
#[cfg(test)]
impl ResetTransform {
    pub(super) fn synthetic() -> Self {
        Self(std::array::from_fn(|i| 0x10203000 + i as u32))
    }
}
impl ResetTransform {
    pub fn capture(image: &PeImage<'_>) -> Result<Self> {
        let mut values = [0; 12];
        for (i, value) in values.iter_mut().enumerate() {
            *value = word(image.at(0x559730 + i as u32 * 4, 4)?, 0);
        }
        Ok(Self(values))
    }
}

/// Non-owning resolved binding to selection's actual pavement definition entry.
/// No caller-supplied native handle/reference/count/definition; no Arc authority.
pub struct PavementModel {
    binding: ResolvedBinding,
}
impl PavementModel {
    pub fn validate(&self) -> Result<()> {
        self.binding.definition().map(|_| ())
    }
    pub fn validate_source(
        &self,
        source: &crate::live_host::camera_bridge::PropertySource,
    ) -> Result<()> {
        self.validate()?;
        if self.binding.handle() != source.resource() {
            return Err("property source pavement resource registration mismatch".into());
        }
        Ok(())
    }
    pub fn bind(assets: &crate::selection::Assets) -> Result<Self> {
        let owner = assets.pavement_resource();
        if owner.reference_role() != ReferenceRole::NativeStatic688b48 {
            return Err("unsupported pavement reference role".into());
        }
        let binding = owner.binding()?;
        owner.resolve(&binding)?; // directory + slot + generation + occupancy
        Ok(Self { binding })
    }
}

/// Explicitly tagged MODERN allocation identity, never native pointer bits.
/// Slot equals append offset only INSIDE its pool/allocation generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle {
    pool: u64,
    allocation: u64,
    slot: u32,
}

/// ONE live688740 analogue. No Clone, public mutable Vec, or fabricated base.
/// Distinct from appearance688388/kind13. Its producer remains root-owned.
pub struct Pool {
    id: u64,
    allocation: u64,
    model: ResolvedBinding,
    records: Option<Vec<Descriptor>>,
    source_epoch: Option<crate::live_host::camera_bridge::PropertyEpoch>,
    source_high_water: Option<crate::live_host::camera_bridge::PropertyEpoch>,
    #[cfg(test)]
    fail_next_allocation: bool,
}
impl Pool {
    /// Read-only admission/retirement observation; no source authority minted.
    pub(crate) fn lifecycle_observation(
        &self,
    ) -> (
        u64,
        u64,
        Option<crate::live_host::camera_bridge::PropertyEpoch>,
        Option<usize>,
    ) {
        (
            self.id,
            self.allocation,
            self.source_epoch,
            self.records.as_ref().map(Vec::len),
        )
    }
    #[cfg(test)]
    pub(crate) fn test_identity(&self) -> (u64, u64) {
        (self.id, self.allocation)
    }
    #[cfg(test)]
    pub(crate) fn test_is_retired(&self) -> bool {
        self.records.is_none()
    }
    #[cfg(test)]
    pub(crate) fn test_exhaust_generation(&mut self) {
        self.allocation = u64::MAX;
    }
    #[cfg(test)]
    pub(crate) fn test_fail_allocation(&mut self) {
        self.fail_next_allocation = true;
    }
    pub fn build_from_install(
        input: &crate::live_host::camera_bridge::PropertySource,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<Self> {
        model.validate_source(input)?;
        let mut pool = Self::from_rows(input.rows(), input.count(), model, reset)?;
        pool.admit_source_epoch(input.epoch(), false)?;
        pool.source_epoch = Some(input.epoch());
        Ok(pool)
    }
    pub fn rebuild_from_install(
        &mut self,
        input: &crate::live_host::camera_bridge::PropertySource,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<()> {
        // A replacement attempt invalidates old descriptors before any binding read.
        self.retire_source_frame()?;
        self.admit_source_epoch(input.epoch(), false)?;
        model.validate_source(input)?;
        self.populate_rows(input.rows(), input.count(), model, reset)?;
        self.source_epoch = Some(input.epoch());
        Ok(())
    }
    pub fn build(world: &World, model: &PavementModel, reset: &ResetTransform) -> Result<Self> {
        let mut pool = Self::from_rows(
            &world.categories,
            world.topology.count617624(),
            model,
            reset,
        )?;
        let epoch = crate::live_host::camera_bridge::PropertyEpoch::World(world.topology.epoch());
        pool.admit_source_epoch(epoch, false)?;
        pool.source_epoch = Some(epoch);
        Ok(pool)
    }
    /// Event-thread path: reuse the EXISTING protected World capture carried
    /// by the acknowledged upper supplier. This is an immutable input, not
    /// another descriptor/selection authority or a request to the worker.
    pub fn build_from_protected(
        input: &crate::navigator::upper::Inputs,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<Self> {
        let mut pool = Self::from_rows(&input.rows, input.count, model, reset)?;
        pool.admit_source_epoch(input.source_epoch, false)?;
        pool.source_epoch = Some(input.source_epoch);
        Ok(pool)
    }
    fn from_rows(
        rows: &[[u8; 380]; 52],
        count: i32,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<Self> {
        let mut pool = Self {
            id: identity()?,
            allocation: 0,
            model: model.binding.clone(),
            records: None,
            source_epoch: None,
            source_high_water: None,
            #[cfg(test)]
            fail_next_allocation: false,
        };
        pool.rebuild_rows(rows, count, model, reset)?;
        Ok(pool)
    }
    /// Call at actual44faf0 writer, never on every UI refresh. Prior handles
    /// retire even on an unsupported replacement input/allocation failure.
    pub fn rebuild(
        &mut self,
        world: &World,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<()> {
        let epoch = crate::live_host::camera_bridge::PropertyEpoch::World(world.topology.epoch());
        self.begin_source_rebuild(epoch)?;
        self.populate_rows(
            &world.categories,
            world.topology.count617624(),
            model,
            reset,
        )?;
        self.source_epoch = Some(epoch);
        Ok(())
    }
    pub fn rebuild_from_protected(
        &mut self,
        input: &crate::navigator::upper::Inputs,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<()> {
        self.begin_source_rebuild(input.source_epoch)?;
        self.populate_rows(&input.rows, input.count, model, reset)?;
        self.source_epoch = Some(input.source_epoch);
        Ok(())
    }
    /// Shared by the World and protected-input writers. Only a source active
    /// BEFORE this call's own allocation invalidation can rebuild at equality.
    /// Explicit retirement or a failed replacement cannot convey that authority
    /// through a detached capture. Invalidate before any fallible replacement.
    fn begin_source_rebuild(
        &mut self,
        epoch: crate::live_host::camera_bridge::PropertyEpoch,
    ) -> Result<()> {
        let was_active = self.source_epoch == Some(epoch);
        self.retire_source_frame()?;
        self.admit_source_epoch(epoch, was_active)
    }
    /// Retained even while allocation is retired. A detached old protected
    /// frame cannot become a new source merely because its rows compare equal.
    fn admit_source_epoch(
        &mut self,
        epoch: crate::live_host::camera_bridge::PropertyEpoch,
        allow_same_world_rebuild: bool,
    ) -> Result<()> {
        if let Some(old) = self.source_high_water {
            if !epoch.follows(old) && !(allow_same_world_rebuild && epoch == old) {
                return Err("old or duplicate property Pool source epoch".into());
            }
        }
        self.source_high_water = Some(epoch);
        Ok(())
    }
    /// Source/session replacement invalidation; not the 10Hz camera/render tick.
    /// Root must call before discarding/replacing the protected World supplier.
    pub fn retire_source_frame(&mut self) -> Result<()> {
        self.records = None;
        self.source_epoch = None;
        self.allocation = self
            .allocation
            .checked_add(1)
            .ok_or("descriptor generation exhausted")?;
        Ok(())
    }
    fn rebuild_rows(
        &mut self,
        rows: &[[u8; 380]; 52],
        count: i32,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<()> {
        self.retire_source_frame()?;
        self.populate_rows(rows, count, model, reset)
    }
    fn populate_rows(
        &mut self,
        rows: &[[u8; 380]; 52],
        count: i32,
        model: &PavementModel,
        reset: &ResetTransform,
    ) -> Result<()> {
        let definition = model.binding.definition()?;
        let count = usize::try_from(count)
            .map_err(|_| "negative native descriptor allocation count outside safe domain")?;
        if count > rows.len() {
            return Err("native descriptor count exceeds live World52 rows".into());
        }
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_allocation) {
            return Err("pavement descriptor allocation failed".into());
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| "pavement descriptor allocation failed")?;
        // The one stack template is reset and appended; carry partial bits.
        let mut template = Descriptor::construct(reset);
        for (index, row) in rows.iter().take(count).enumerate() {
            template.reset(reset);
            template.set_model(&definition);
            template.put_word(4, word(row, 0xe0));
            template.put_word(0xc, word(row, 0xe4));
            let color = word(row, 0x94);
            template.put_word(0x3c, color);
            template.put_word(0x40, color);
            template.put_half(0x30, 4);
            template.put_word(0x38, index as u32);
            if row[0x90] & 1 != 0 {
                template.bytes[0xb8] = template.bytes[0xb8].and_or(0xfe, 0);
            }
            records.push(Descriptor {
                bytes: template.bytes,
            }); //71-word native append
            //44faf0 also resets the template/builds kind13 in the OTHER pool.
            //It cannot alter the remaining UNKNOWN bits or these reset stores.
            //This lane does not claim that side's mesh/removal/renderer effects.
        }
        self.model = model.binding.clone();
        self.records = Some(records);
        Ok(())
    }
    pub fn count(&self) -> Result<usize> {
        self.model.definition()?;
        Ok(self
            .records
            .as_ref()
            .ok_or("descriptor source frame retired")?
            .len())
    }
    ///4dfba0's signed index<count is unsafe for negatives. This is an explicit
    ///safe admission domain, NOT a claim that native negatives yield null.
    pub fn lookup(&self, index: i32) -> Result<Option<Handle>> {
        let index = u32::try_from(index).map_err(|_| "negative4dfba0 index outside safe domain")?;
        if index as usize >= self.count()? {
            return Ok(None);
        }
        Ok(Some(Handle {
            pool: self.id,
            allocation: self.allocation,
            slot: index,
        }))
    }
    pub fn resolve(&self, handle: Handle) -> Result<&Descriptor> {
        self.model.definition()?; // old descriptor aliases cannot outlive occupied resource
        if handle.pool != self.id || handle.allocation != self.allocation {
            return Err("foreign or stale property descriptor allocation".into());
        }
        self.records
            .as_ref()
            .ok_or("descriptor source frame retired")?
            .get(handle.slot as usize)
            .ok_or_else(|| "property descriptor slot outside current allocation".into())
    }
    /// A kind/category facade for existing A/property algorithms; identity is
    /// typed. No native-key conversion or display hit is fabricated.
    pub fn selected(&self, handle: Handle) -> Result<Selected> {
        let d = self.resolve(handle)?;
        Ok(Selected {
            key: SelectionKey::PropertyDescriptor(handle),
            kind: d.kind()?,
            category: d.word(0x38)?,
            hit: None,
        })
    }
    pub fn resolve_selected(&self, selected: Selected) -> Result<&Descriptor> {
        let SelectionKey::PropertyDescriptor(handle) = selected.key else {
            return Err("current selection is outside property descriptor pool".into());
        };
        let d = self.resolve(handle)?;
        if selected.kind != d.kind()? || selected.category != d.word(0x38)? {
            return Err("selection facade differs from actual descriptor".into());
        }
        Ok(d)
    }
    /// A single actual write to ROOT's current822d50. Call twice with a fresh
    ///selector/read and the distinct transient method at native F sites.
    pub fn lookup_publish_current(&self, selection: &mut SelectionState, index: i32) -> Result<()> {
        let value = self.lookup(index)?.map(|h| self.selected(h)).transpose()?;
        selection.current = value;
        Ok(())
    }
    pub fn lookup_publish_transient(
        &self,
        selection: &mut SelectionState,
        index: i32,
    ) -> Result<()> {
        let value = self.lookup(index)?.map(|h| self.selected(h)).transpose()?;
        selection.transient = value;
        Ok(())
    }
    /// Call only AFTER mode0 has actually completed. Reloads ROOT's current,
    /// never transient/hovered/cached pre-mode state. Does not mutate camera.
    pub fn current_camera(
        &self,
        selection: &SelectionState,
        world: &World,
        model: &PavementModel,
        assets: &crate::selection::Assets,
        height: u32,
        radius: u32,
    ) -> Result<BlockInitialization> {
        if self.source_epoch.is_some_and(|epoch| {
            epoch != crate::live_host::camera_bridge::PropertyEpoch::World(world.topology.epoch())
        }) {
            return Err("property camera World epoch mismatch".into());
        }
        self.camera_rows(
            selection.current,
            &world.categories,
            world.topology.count617624(),
            model,
            assets,
            height,
            radius,
        )
    }
    /// Existing immutable upper payload at the SAME suspended Host boundary;
    /// do not reconstruct it from Scene or reuse one from a prior boundary.
    pub fn current_camera_from_protected(
        &self,
        selection: &SelectionState,
        input: &crate::navigator::upper::Inputs,
        model: &PavementModel,
        assets: &crate::selection::Assets,
        height: u32,
        radius: u32,
    ) -> Result<BlockInitialization> {
        if self
            .source_epoch
            .is_some_and(|epoch| epoch != input.source_epoch)
        {
            return Err("property camera protected World epoch mismatch".into());
        }
        self.camera_rows(
            selection.current,
            &input.rows,
            input.count,
            model,
            assets,
            height,
            radius,
        )
    }
    fn camera_rows(
        &self,
        current: Option<Selected>,
        rows: &[[u8; 380]; 52],
        count: i32,
        model: &PavementModel,
        assets: &crate::selection::Assets,
        height: u32,
        radius: u32,
    ) -> Result<BlockInitialization> {
        let selected = current.ok_or("post-mode current822d50 is null")?;
        let d = self.resolve_selected(selected)?;
        if d.kind()? != 4 || d.word(0)? != PAVEMENT_REFERENCE {
            return Err("current descriptor is not category pavement".into());
        }
        if self.model.handle() != model.binding.handle() {
            return Err("post-mode pavement model/resource generation changed".into());
        }
        // A binding from another directory or a retired/replaced occupied slot
        // rejects even if all116 bytes compare equal. No cached Arc proof.
        let asset_definition = assets.pavement_resource().resolve(&model.binding)?;
        if count < 0 || count as usize != self.count()? {
            return Err("current category count differs from descriptor allocation".into());
        }
        let category = d.word(0x38)?;
        let row = rows
            .get(category as usize)
            .ok_or("current descriptor category outside World")?;
        if category >= count as u32 {
            return Err("current descriptor category outside native count".into());
        }
        let block = assets
            .blocks
            .iter()
            .find(|b| b.id == category as usize)
            .ok_or("current descriptor has no immutable topology block")?;
        if word(row, 0) != category
            || [word(row, 0xe0), word(row, 0xe4)] != block.center.map(f32::to_bits)
            || [word(row, 0xd0), word(row, 0xd4)] != block.size.map(f32::to_bits)
            || d.translation()? != [word(row, 0xe0), 0, word(row, 0xe4)]
        {
            return Err(
                "post-mode descriptor/live World/immutable topology geometry mismatch".into(),
            );
        }
        if d.bytes[0xb8].bit(1) != Some(row[0x90] & 1 == 0) {
            return Err("current category active flag differs from descriptor".into());
        }
        for i in 0..29 {
            if d.word(0x44 + i * 4)? != word(&asset_definition, i * 4) {
                return Err("descriptor/current pavement model definition mismatch".into());
            }
        }
        // All identity/translation/model/topology joins have now passed.
        // Existing method retains separate translation and model-center words.
        assets.camera_block_initialization(category, height, radius)
    }
}

#[cfg(test)]
#[path = "property_descriptor_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "property_lifetime_consumer_tests.rs"]
mod consumer_tests;

// Selection-free entry regressions need only this module's authored Pool fixture.
#[cfg(test)]
#[path = "selection_free_path_tests.rs"]
mod selection_free_path_tests;

#[cfg(test)]
mod checked_identity_tests {
    use super::*;
    #[test]
    fn checked_pool_identity_exhaustion_never_wraps_or_revives_aliases() {
        let next = AtomicU64::new(u64::MAX - 1);
        assert_eq!(identity_from(&next).unwrap(), u64::MAX - 1);
        assert_eq!(
            identity_from(&next).unwrap_err(),
            "modern descriptor identity space exhausted"
        );
        assert_eq!(
            identity_from(&next).unwrap_err(),
            "modern descriptor identity space exhausted"
        );
    }
}
