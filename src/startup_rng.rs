//! First-process static RNG reseed boundary; not a registry for later AI nodes.
//! The host supplies the selected seed. Returned values are initialization
//! snapshots, not parallel authoritative copies of live simulation RNG state.

/// CRT table order533004..5331e4, including embedded UI/deck streams and
/// the two streams constructed after the process stream.
pub const REFERENCES: [u32; 29] = [
    0x5607f0, 0x560a08, 0x566560, 0x5904f0, 0x590f68, 0x617610, 0x61c538, 0x6744c0, 0x659338,
    0x68c028, 0x6ae190, 0x6fc8a8, 0x6fc8c0, 0x6fcd40, 0x7a4190, 0x7f3db8, 0x7f9038, 0x801df8,
    0x822ae0, 0x8693a0, 0x8af3ec, 0x8b06e8, 0x8b2250, 0x9341b0, 0x93a290, 0x93aee8, 0x93c818,
    0x940cf8, 0x95c300,
];

pub const HEAD: u32 = REFERENCES[0];
pub const TAIL: u32 = REFERENCES[REFERENCES.len() - 1];
pub const PROCESS: u32 = 0x93c818;

pub struct Snapshot {
    pub states: [u32; 29],
    pub process: [u32; 3],
    pub last_seed: u32,
}
impl Snapshot {
    /// After CRT construction, before dynamic player/account registration.
    /// Includes4e55b6's process-node alias. No wall clock or replay policy here.
    pub fn from_seed(seed: u32) -> Self {
        let mut registry = crate::player_manager::RandomRegistry {
            process: [0, 0x343fd, 0x269ec3],
            head: HEAD,
            tail: TAIL,
        };
        let mut states = [0; 29];
        let mut last_seed = 0;
        registry
            .reseed_with_process_node(seed, &mut last_seed, Some(PROCESS), |reference, state| {
                let index = REFERENCES
                    .iter()
                    .position(|value| *value == reference)
                    .ok_or("unknown static RNG node")?;
                states[index] = state;
                Ok(REFERENCES.get(index + 1).copied().unwrap_or(0))
            })
            .expect("fixed static registry links");
        // Later CRT nodes advance the process stream after its self-alias store.
        states[REFERENCES.iter().position(|&r| r == PROCESS).unwrap()] = registry.process[0];
        Self {
            states,
            process: registry.process,
            last_seed,
        }
    }

    pub fn state(&self, reference: u32) -> u32 {
        self.states[REFERENCES
            .iter()
            .position(|&r| r == reference)
            .expect("known static RNG reference")]
    }

    /// Transfer the five initial states into the owning resident subsystem.
    /// Other stream snapshots remain available for their respective owners.
    pub fn resident_creation(
        &self,
        auxiliary: crate::resident::AuxiliaryPool,
    ) -> crate::economy::ResidentCreationState {
        crate::economy::ResidentCreationState {
            auxiliary,
            preference_rng: self.state(0x6ae190),
            initialization_rng: crate::resident::InitializationRng {
                movement: self.state(0x6fc8a8),
                constructor: self.state(0x6fcd40),
                objective: self.state(0x7a4190),
            },
            identity_rng: self.state(0x6fc8c0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::REFERENCES;
    #[test]
    #[ignore = "requires original static registry reseed observations"]
    fn matches_original_static_registry() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_STATIC_RNG")).unwrap();
        assert_eq!(&bytes[..8], b"MTSTRNG2");
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), REFERENCES.len() as u32);
        let mut at = 16;
        for _ in 0..word(8) {
            let snapshot = super::Snapshot::from_seed(word(at));
            at += 4;
            for (reference, state) in REFERENCES.into_iter().zip(snapshot.states) {
                assert_eq!(reference, word(at));
                at += 4;
                assert_eq!(state, word(at));
                at += 4;
            }
            for value in snapshot.process {
                assert_eq!(value, word(at));
                at += 4;
            }
            assert_eq!(snapshot.last_seed, word(at));
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }
}
