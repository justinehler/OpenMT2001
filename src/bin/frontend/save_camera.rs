//! Legacy UI save snapshots, distinct from the native camera bridge.
//! One UI writer publishes a complete immutable pair/mode; workers never call
//! back into the UI. A source fence orders file publication against retirement.
use openmt2001::{
    camera::bank::{Bank, Capture, StrategicTransition},
    flyby::CameraSample,
    level_startup::SourceEpoch,
};
use std::sync::{Arc, Mutex, MutexGuard};

/// UI-owned raw storage. The caller moves a real process Bank here, or restores
/// supplied records into that storage. No default/fresh initialization policy
/// is hidden in this adapter; warm refresh moves the same value.
pub(crate) struct LegacyPair {
    bank: Bank,
    kind_dirty: u8,
}
impl LegacyPair {
    pub(crate) fn new(bank: Bank, kind_dirty: u8) -> Self { Self { bank, kind_dirty } }
    pub(crate) fn capture(&self) -> Capture { self.bank.capture() }
    pub(crate) fn primary_kind(&self) -> u32 { self.bank.primary_kind() }
    pub(crate) fn kind_dirty(&self) -> u8 { self.kind_dirty }
    /// Transport both raw rows without deriving the separate dirty byte.
    pub(crate) fn restore(&mut self, capture: Capture) { self.bank.restore(capture); }
    /// Existing legacy presentation projection only. It does not implement
    /// numeric camera callbacks/common/sample state or touch the return record.
    pub(crate) fn project_primary(&mut self, camera: CameraSample) -> Result<(), String> {
        if camera.eye.into_iter().chain(camera.target).chain(camera.up).any(|v| !v.is_finite())
            || camera.up == [0.; 3]
        {
            return Err("Cannot publish an invalid UI camera pose".into());
        }
        self.bank.try_update_primary(|raw| {
            for (at, triple) in [(0x4c, camera.eye), (0x58, camera.target), (0x64, camera.up)] {
                for (i, value) in triple.into_iter().enumerate() {
                    raw[at + 4*i..at + 4*i + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
            Ok(())
        })
    }
    /// The caller completes the real UI clear before this native operation.
    /// RAW primary kind4 chooses return, independently of host metadata/tuples.
    /// Preflight any required return rendering/control work before mutation.
    pub(crate) fn strategic_after_ui_clear(&mut self) -> StrategicTransition {
        self.bank.strategic_after_ui_clear(&mut self.kind_dirty)
    }
}

struct State {
    epoch: u64,
    retired: bool,
    latest: Option<(Capture, u32)>,
}
type Shared = Arc<Mutex<State>>;
fn lock(shared: &Shared) -> Result<MutexGuard<'_, State>, String> {
    shared.lock().map_err(|_| "UI camera snapshot source poisoned".into())
}
fn live(state: &State, epoch: u64) -> Result<(), String> {
    if state.epoch != epoch { return Err("UI camera snapshot belongs to another World".into()); }
    if state.retired { return Err("UI camera snapshot source retired".into()); }
    Ok(())
}
fn validate(capture: Capture, mode: u32) -> Result<(), String> {
    if !matches!(mode, 0 | 3) { return Err("Unsupported UI camera snapshot interface mode".into()); }
    let kind = u32::from_le_bytes(capture.records()[0][..4].try_into().unwrap());
    // CityView changes interface mode to0 without a raw camera setter/copy.
    // Consequently normal interface mode with raw kind4 is a valid native cut.
    if mode == 3 && kind != 4 {
        return Err("UI camera raw kind disagrees with host strategic mode".into());
    }
    Ok(())
}

/// Exactly one publisher; deliberately not Clone. Drop retires its source.
pub(crate) struct Publisher { shared: Shared }
#[derive(Clone)]
pub(crate) struct Reader { shared: Shared }
/// By-value cut. The private origin prevents same-epoch channel substitution.
#[derive(Clone)]
pub(crate) struct Snapshot {
    origin: Shared,
    epoch: u64,
    capture: Capture,
    interface_mode: u32,
}
impl Snapshot {
    pub(crate) fn epoch(&self) -> u64 { self.epoch }
    pub(crate) fn capture(&self) -> Capture { self.capture }
    pub(crate) fn interface_mode(&self) -> u32 { self.interface_mode }
}

/// Epoch comes only from the retained Session's actual topology initialization.
pub(crate) fn channel(epoch: SourceEpoch) -> (Publisher, Reader) {
    channel_serial(epoch.serial())
}
fn channel_serial(epoch: u64) -> (Publisher, Reader) {
    let shared = Arc::new(Mutex::new(State { epoch, retired: false, latest: None }));
    (Publisher { shared: Arc::clone(&shared) }, Reader { shared })
}
impl Publisher {
    pub(crate) fn publish(&mut self, capture: Capture, interface_mode: u32) -> Result<(), String> {
        let mut state = lock(&self.shared)?;
        live(&state, state.epoch)?;
        if let Err(error) = validate(capture, interface_mode) {
            state.latest = None;
            return Err(error);
        }
        state.latest = Some((capture, interface_mode));
        Ok(())
    }
    /// A failed UI projection/unsupported transaction must not leave an older
    /// published sample available as though it described the current UI state.
    pub(crate) fn invalidate(&mut self) -> Result<(), String> {
        let mut state = lock(&self.shared)?;
        live(&state, state.epoch)?;
        state.latest = None;
        Ok(())
    }
    pub(crate) fn retire(&mut self) -> Result<(), String> {
        let mut state = lock(&self.shared)?;
        state.retired = true;
        state.latest = None;
        Ok(())
    }
}
impl Drop for Publisher {
    fn drop(&mut self) { let _ = self.retire(); }
}
impl Reader {
    pub(crate) fn read(&self, epoch: SourceEpoch) -> Result<Snapshot, String> {
        self.read_serial(epoch.serial())
    }
    pub(crate) fn check_epoch(&self, epoch: SourceEpoch) -> Result<(), String> {
        let state = lock(&self.shared)?;
        live(&state, epoch.serial())
    }
    pub(crate) fn read_serial(&self, epoch: u64) -> Result<Snapshot, String> {
        let state = lock(&self.shared)?;
        live(&state, epoch)?;
        let (capture, interface_mode) = state.latest.ok_or("UI camera snapshot is not published")?;
        Ok(Snapshot { origin: Arc::clone(&self.shared), epoch, capture, interface_mode })
    }
    /// Only final file publication runs under this fence, after encoding and
    /// staging/flush. No UI callback, producer or simulation work may run here.
    /// A newer same-source UI sample does not invalidate an already captured cut.
    pub(crate) fn with_live<T>(&self, snapshot: &Snapshot,
        publish: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if !Arc::ptr_eq(&self.shared, &snapshot.origin) {
            return Err("UI camera snapshot came from another source".into());
        }
        let state = lock(&self.shared)?;
        live(&state, snapshot.epoch)?;
        let result = publish();
        drop(state);
        result
    }
}

#[cfg(test)]
#[path = "save_camera_tests.rs"]
mod tests;
