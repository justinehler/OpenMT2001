//! Live resident service provenance, carried on the existing operation stream.
//! No resident/auxiliary bytes or player roster are captured here.
use super::PropertyEpoch;
use openmt2001::resident::{AuxiliaryBinding, AuxiliaryPool, AuxiliarySource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidentSource {
    epoch: PropertyEpoch,
    pool: AuxiliarySource,
}
impl ResidentSource {
    pub fn capture(session: &crate::fresh_city::Session<'_>) -> Self {
        Self::from_world(&session.world)
    }
    pub fn from_world(world: &openmt2001::level_startup::World) -> Self {
        Self {
            epoch: PropertyEpoch::World(world.topology.epoch()),
            pool: world.creation.auxiliary.source(),
        }
    }
    pub fn epoch(&self) -> PropertyEpoch {
        self.epoch
    }
    pub fn validate_live(&self) -> Result<(), String> {
        self.pool.validate_live()
    }
    pub fn validate(&self, expected: PropertyEpoch, pool: &AuxiliaryPool) -> Result<(), String> {
        if self.epoch != expected {
            return Err("resident service World source epoch mismatch".into());
        }
        self.pool.validate_pool(pool)
    }
    pub fn validate_binding(&self, binding: &AuxiliaryBinding) -> Result<(), String> {
        binding.validate_source(&self.pool)
    }
    #[cfg(test)]
    pub(crate) fn synthetic(epoch: u64, pool: &AuxiliaryPool) -> Self {
        Self {
            epoch: PropertyEpoch::synthetic(epoch),
            pool: pool.source(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidentRead {
    Count7a18ec,
    Flags336(i32),
    /// Native4dfba0's signed argument indexes6899a0 directly. It is not a
    /// resident slot/+148/+14c/+38 translation or player index conversion.
    Lookup6899a0(i32),
    Kind30(AuxiliaryBinding),
    Index38(AuxiliaryBinding),
    Validate(AuxiliaryBinding),
    /// Validate the reached identity and expose the concrete missing callback.
    /// A model field copied from Selected or a render hit cannot satisfy this.
    Participant481a50(AuxiliaryBinding),
}
