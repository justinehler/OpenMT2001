//! Process-retained registry of weak ACTUAL6899a0 allocations. Handles are
//! opaque, checked modern identities, never fabricated native32 addresses.
use super::{
    continuation::ParticipantDescriptor,
    ui_selection::{Selected, SelectionKey},
};
use crate::live_host::camera_bridge::{PropertyEpoch, ResidentSource};
use openmt2001::resident::AuxiliaryBinding;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};
type Result<T> = std::result::Result<T, String>;
static NEXT_REGISTRY: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    registry: u64,
    generation: u64,
    entry: u64,
}
pub struct Registry {
    id: u64,
    generation: u64,
    next: u64,
    source: Option<ResidentSource>,
    last_epoch: Option<PropertyEpoch>,
    entries: BTreeMap<u64, AuxiliaryBinding>,
    failure: Option<String>,
}
impl Registry {
    pub fn new() -> Result<Self> {
        let id = NEXT_REGISTRY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| "resident registry identity exhausted")?;
        Ok(Self {
            id,
            generation: 1,
            next: 0,
            source: None,
            last_epoch: None,
            entries: BTreeMap::new(),
            failure: None,
        })
    }
    pub fn validate_install(&self, source: &ResidentSource, channel_generation: u64) -> Result<()> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        source.validate_live()?;
        if source.epoch().serial() != channel_generation {
            return Err("resident source/channel World epoch mismatch".into());
        }
        if self.source.is_some() {
            return Err("resident source active; refresh must retain SAME registry".into());
        }
        if self
            .last_epoch
            .is_some_and(|old| !source.epoch().follows(old))
        {
            return Err(
                "retired/duplicate resident World epoch cannot revive detached capture".into(),
            );
        }
        Ok(())
    }
    pub fn install(&mut self, source: &ResidentSource, channel_generation: u64) -> Result<()> {
        self.validate_install(source, channel_generation)?;
        self.last_epoch = Some(source.epoch());
        self.source = Some(source.clone());
        Ok(())
    }
    /// Invalidate before Inbox retirement wakes Host. High-water World epoch
    /// and this registry identity remain retained on the same process Owner.
    pub fn retire(&mut self) {
        self.source = None;
        self.entries.clear();
        match self.generation.checked_add(1) {
            Some(next) => self.generation = next,
            None => self.fail("resident registry generation exhausted".into()),
        }
    }
    pub fn fail(&mut self, error: String) {
        self.source = None;
        self.entries.clear();
        if self.failure.is_none() {
            self.failure = Some(error);
        }
    }
    pub fn source(&self) -> Result<&ResidentSource> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let source = self
            .source
            .as_ref()
            .ok_or("resident registry source retired/not installed")?;
        source.validate_live()?;
        Ok(source)
    }
    pub fn binding(&self, handle: Handle) -> Result<&AuxiliaryBinding> {
        let source = self.source()?;
        if handle.registry != self.id || handle.generation != self.generation {
            return Err("foreign/stale resident registry handle".into());
        }
        let binding = self
            .entries
            .get(&handle.entry)
            .ok_or("resident handle has no issued allocation")?;
        source.validate_binding(binding)?;
        Ok(binding)
    }
    pub fn selected(
        &mut self,
        binding: AuxiliaryBinding,
        kind: i16,
        index38: u32,
    ) -> Result<ParticipantDescriptor> {
        self.source()?.validate_binding(&binding)?;
        let entry = match self.entries.iter().find(|(_, actual)| **actual == binding) {
            Some((entry, _)) => *entry,
            None => {
                let entry = self.next;
                self.next = self
                    .next
                    .checked_add(1)
                    .ok_or("resident handle sequence exhausted")?;
                self.entries.insert(entry, binding);
                entry
            }
        };
        let value = Selected {
            key: SelectionKey::ResidentDescriptor(Handle {
                registry: self.id,
                generation: self.generation,
                entry,
            }),
            kind,
            category: index38,
            hit: None,
        };
        ParticipantDescriptor::from_registered(value, self)
    }
    pub fn validate_selected(&self, value: Selected) -> Result<()> {
        let SelectionKey::ResidentDescriptor(handle) = value.key else {
            return Err("selection is not actual resident auxiliary origin".into());
        };
        self.binding(handle)?;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn identity(&self) -> (u64, u64) {
        (self.id, self.generation)
    }
}

#[cfg(test)]
#[path = "resident_descriptor_tests.rs"]
mod tests;
