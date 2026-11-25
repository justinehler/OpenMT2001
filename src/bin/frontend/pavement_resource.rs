//! Finite modern pavement definition resource. No native manager/render facade.
//! One Directory owns its occupied CPU definition and exact loaded source bytes.
//! Bindings hold Weak references: possession is never evidence of occupancy.
use std::sync::{Arc, Mutex, Weak, atomic::{AtomicU64, Ordering}};
use openmt2001::archive::{ArchiveEntry, ArchiveIndex};

type Result<T> = std::result::Result<T, String>;
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);
pub(super) const PATH: &str = "gamedata2/New_building stuff/pavement.tm";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirectoryId(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Generation(u64);
/// Slot in THIS finite modern directory, never a native returned model handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PavementSlot { Definition }
/// Explicitly local/domain-specific. Cannot be constructed from native integers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalPavementHandle {
    directory: DirectoryId,
    generation: Generation,
    slot: PavementSlot,
}
/// Native address identifies a static reference ROLE only, not an allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceRole { NativeStatic688b48 }

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArchiveProvenance {
    pub header_words: [u32;8],
    pub entry_index: usize,
    pub archive_length: u64,
    pub entry: ArchiveEntry,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Provenance {
    Archive(ArchiveProvenance),
    #[cfg(test)]
    SyntheticFixture,
}
/// Only selection (the parent module) can construct this from its successful
/// mesh parse/model_definition. No parse, scene lookup or numeric conversion.
pub(super) struct LoadedPavement {
    definition: [u8;116],
    provenance: Provenance,
    source_bytes: Box<[u8]>,
}
impl LoadedPavement {
    pub(super) fn from_loaded(directory: &ArchiveIndex, archive: &[u8],
        entry: &ArchiveEntry, definition: [u8;116]) -> Result<Self> {
        let found = directory.find(PATH).ok_or("missing loaded pavement archive entry")?;
        if !std::ptr::eq(found, entry) {
            return Err("pavement source is not the selected archive entry".into());
        }
        let entry_index = directory.entries.iter().position(|e| std::ptr::eq(e, entry))
            .ok_or("pavement entry outside supplied directory")?;
        let start = usize::try_from(entry.offset).map_err(|_| "pavement offset outside host domain")?;
        let end = entry.offset.checked_add(entry.length).ok_or("pavement source range overflow")?;
        let end = usize::try_from(end).map_err(|_| "pavement end outside host domain")?;
        let bytes = archive.get(start..end).ok_or("pavement source outside archive")?;
        Ok(Self { definition, source_bytes: bytes.into(),
            provenance: Provenance::Archive(ArchiveProvenance {
                header_words: directory.header_words, entry_index,
                archive_length: archive.len() as u64, entry: entry.clone(),
            }) })
    }
    #[cfg(test)]
    fn synthetic(definition: [u8;116]) -> Self {
        Self { definition, provenance: Provenance::SyntheticFixture, source_bytes: Box::new([]) }
    }
}
struct Entry { handle: LocalPavementHandle, resource: LoadedPavement }
enum Occupancy { Vacant, Occupied(Entry) }
struct State { id: DirectoryId, generation: Generation, occupancy: Occupancy }
impl State {
    fn entry(&self, handle: LocalPavementHandle) -> Result<&Entry> {
        if self.id != handle.directory || self.generation != handle.generation {
            return Err("foreign or stale pavement resource generation".into());
        }
        match &self.occupancy {
            Occupancy::Occupied(entry) if entry.handle == handle => Ok(entry),
            _ => Err("pavement resource slot is vacant".into()),
        }
    }
    fn retire(&mut self) -> Result<()> {
        // Revoke occupancy even if generation space is exhausted. Never ABA.
        self.occupancy = Occupancy::Vacant;
        self.generation = Generation(self.generation.0.checked_add(1)
            .ok_or("pavement resource generation exhausted")?);
        Ok(())
    }
}

/// One retained binding per installed generation. Clones are aliases, not
/// registrations. The fields are private; callers cannot assert occupancy.
#[derive(Clone)]
pub(crate) struct ResolvedBinding {
    handle: LocalPavementHandle,
    directory: Weak<Mutex<State>>,
}
impl ResolvedBinding {
    pub(crate) fn handle(&self) -> LocalPavementHandle { self.handle }
    pub(crate) fn definition(&self) -> Result<[u8;116]> {
        let directory = self.directory.upgrade().ok_or("pavement resource owner removed")?;
        let state = directory.lock().map_err(|_| "pavement resource directory poisoned")?;
        Ok(state.entry(self.handle)?.resource.definition)
    }
}

/// Sole strong authority, retained in selection::Assets. No Clone/Default.
/// The mutex protects real occupied state; an Arc identity alone grants nothing.
pub(crate) struct Directory {
    state: Arc<Mutex<State>>,
    binding: Option<ResolvedBinding>, // native role's actually populated slot0 analogue
}
impl Directory {
    pub(super) fn install(loaded: LoadedPavement) -> Result<Self> {
        let id = DirectoryId(NEXT_DIRECTORY.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
            |v| v.checked_add(1)).map_err(|_| "pavement directory identity exhausted")?);
        let mut owner = Self { state: Arc::new(Mutex::new(State {
            id, generation: Generation(0), occupancy: Occupancy::Vacant,
        })), binding: None };
        owner.populate(loaded)?;
        Ok(owner)
    }
    fn populate(&mut self, loaded: LoadedPavement) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| "pavement resource directory poisoned")?;
        if !matches!(&state.occupancy, Occupancy::Vacant) {
            return Err("pavement slot must be retired before registration".into());
        }
        state.generation = Generation(state.generation.0.checked_add(1)
            .ok_or("pavement resource generation exhausted")?);
        let handle = LocalPavementHandle { directory: state.id, generation: state.generation,
            slot: PavementSlot::Definition };
        state.occupancy = Occupancy::Occupied(Entry { handle, resource: loaded });
        self.binding = Some(ResolvedBinding { handle, directory: Arc::downgrade(&self.state) });
        Ok(())
    }
    /// Root must first retire dependent Pool allocations at the actual writer.
    /// This revokes every old binding, including equal-definition replacements.
    pub(super) fn replace(&mut self, loaded: LoadedPavement) -> Result<()> {
        self.retire()?;
        self.populate(loaded)
    }
    pub(super) fn retire(&mut self) -> Result<()> {
        self.binding = None;
        self.state.lock().map_err(|_| "pavement resource directory poisoned")?.retire()
    }
    pub(crate) fn binding(&self) -> Result<ResolvedBinding> {
        let binding = self.binding.as_ref().ok_or("pavement reference role has no populated slot")?;
        self.resolve(binding)?;
        Ok(binding.clone())
    }
    pub(crate) fn resolve(&self, binding: &ResolvedBinding) -> Result<[u8;116]> {
        if !Weak::ptr_eq(&binding.directory, &Arc::downgrade(&self.state)) {
            return Err("pavement binding belongs to another directory".into());
        }
        binding.definition()
    }
    pub(crate) fn reference_role(&self) -> ReferenceRole { ReferenceRole::NativeStatic688b48 }
    /// Count comes from actual registration, never an authored native word.
    pub(crate) fn populated_slots(&self) -> Result<usize> {
        let state = self.state.lock().map_err(|_| "pavement resource directory poisoned")?;
        Ok(match &state.occupancy { Occupancy::Vacant => 0, Occupancy::Occupied(_) => 1 })
    }
    pub(crate) fn provenance(&self) -> Result<Provenance> {
        let binding = self.binding.as_ref().ok_or("pavement reference role has no populated slot")?;
        let state = self.state.lock().map_err(|_| "pavement resource directory poisoned")?;
        Ok(state.entry(binding.handle)?.resource.provenance.clone())
    }
    pub(crate) fn retained_source_length(&self) -> Result<usize> {
        let binding = self.binding.as_ref().ok_or("pavement reference role has no populated slot")?;
        let state = self.state.lock().map_err(|_| "pavement resource directory poisoned")?;
        Ok(state.entry(binding.handle)?.resource.source_bytes.len())
    }
    #[cfg(test)]
    pub(super) fn synthetic(definition: [u8;116]) -> Self {
        Self::install(LoadedPavement::synthetic(definition)).unwrap()
    }
    #[cfg(test)]
    pub(super) fn replace_synthetic(&mut self, definition: [u8;116]) -> Result<()> {
        self.replace(LoadedPavement::synthetic(definition))
    }
}

#[cfg(test)]
#[path="pavement_resource_tests.rs"]
mod tests;
