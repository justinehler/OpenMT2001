//! First-process static RNG reseed boundary; not a registry for later AI nodes.
//! The host supplies the selected seed. Returned values are initialization
//! snapshots, not parallel authoritative copies of live simulation RNG state.

/// Logical identity of the card-receiver RNG owned by static player 0x86cfc8.
/// The receiver is heap allocated during CRT construction (0x48a060), so its
/// native RNG address varies. Use the owner's address as its stable identity;
/// this value is not the address of an embedded RNG in the original image.
pub const STATIC_PLAYER_RECEIVER: u32 = 0x86cfc8;

/// CRT initializer order (0x533004..0x5331e4), including the heap receiver,
/// embedded UI/deck streams and two streams constructed after the process one.
pub const REFERENCES: [u32; 30] = [
    0x5607f0,
    0x560a08,
    0x566560,
    0x5904f0,
    0x590f68,
    0x617610,
    0x61c538,
    0x6744c0,
    0x659338,
    0x68c028,
    0x6ae190,
    0x6fc8a8,
    0x6fc8c0,
    0x6fcd40,
    0x7a4190,
    0x7f3db8,
    0x7f9038,
    0x801df8,
    0x822ae0,
    0x8693a0,
    STATIC_PLAYER_RECEIVER,
    0x8af3ec,
    0x8b06e8,
    0x8b2250,
    0x9341b0,
    0x93a290,
    0x93aee8,
    0x93c818,
    0x940cf8,
    0x95c300,
];

pub const HEAD: u32 = REFERENCES[0];
pub const TAIL: u32 = REFERENCES[REFERENCES.len() - 1];
pub const PROCESS: u32 = 0x93c818;

pub struct Snapshot {
    pub states: [u32; REFERENCES.len()],
    pub process: [u32; 3],
    pub last_seed: u32,
}
impl Snapshot {
    /// After CRT construction, before dynamic player/account registration.
    /// Includes the process-node alias at 0x4e55b6. No wall clock or replay policy.
    pub fn from_seed(seed: u32) -> Self {
        let (states, process, last_seed) = reseed(seed, REFERENCES);
        Self {
            states,
            process,
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

// Share the actual reseed traversal with the legacy fixture's reduced order.
fn reseed<const N: usize>(seed: u32, references: [u32; N]) -> ([u32; N], [u32; 3], u32) {
    let mut registry = crate::player_manager::RandomRegistry {
        process: [0, 0x343fd, 0x269ec3],
        head: references[0],
        tail: references[N - 1],
    };
    let mut states = [0; N];
    let mut last_seed = 0;
    registry
        .reseed_with_process_node(seed, &mut last_seed, Some(PROCESS), |reference, state| {
            let index = references
                .iter()
                .position(|value| *value == reference)
                .ok_or("unknown static RNG node")?;
            states[index] = state;
            Ok(references.get(index + 1).copied().unwrap_or(0))
        })
        .expect("fixed static registry links");
    // Later CRT nodes advance the process stream after its self-alias store.
    states[references.iter().position(|&r| r == PROCESS).unwrap()] = registry.process[0];
    (states, registry.process, last_seed)
}

#[cfg(test)]
mod tests {
    use super::{PROCESS, REFERENCES, STATIC_PLAYER_RECEIVER, Snapshot};

    #[test]
    fn seed_zero_reseeds_all_thirty_crt_nodes_in_order() {
        assert_eq!(
            REFERENCES,
            [
                0x5607f0,
                0x560a08,
                0x566560,
                0x5904f0,
                0x590f68,
                0x617610,
                0x61c538,
                0x6744c0,
                0x659338,
                0x68c028,
                0x6ae190,
                0x6fc8a8,
                0x6fc8c0,
                0x6fcd40,
                0x7a4190,
                0x7f3db8,
                0x7f9038,
                0x801df8,
                0x822ae0,
                0x8693a0,
                STATIC_PLAYER_RECEIVER,
                0x8af3ec,
                0x8b06e8,
                0x8b2250,
                0x9341b0,
                0x93a290,
                0x93aee8,
                PROCESS,
                0x940cf8,
                0x95c300,
            ]
        );
        let snapshot = Snapshot::from_seed(0);
        // Derived from the LCG, not a capture: the first twenty states are
        // unchanged; the receiver consumes draw 21, shifting later streams.
        // At PROCESS, the output aliases the master state before two more draws.
        assert_eq!(
            snapshot.states,
            [
                38, 7719, 21238, 2437, 8855, 11797, 8365, 32285, 10450, 30612, 5853, 28100, 1142,
                281, 20537, 15921, 8945, 26285, 2997, 14680, 20976, 31891, 21655, 25906, 18457,
                1323, 28881, 0x7eb5dd3a, 7353, 32437,
            ]
        );
        assert_eq!(snapshot.state(STATIC_PLAYER_RECEIVER), 20976);
        assert_eq!(snapshot.process, [0x7eb5dd3a, 0x343fd, 0x269ec3]);
        assert_eq!(snapshot.state(PROCESS), snapshot.process[0]);
        assert_eq!(snapshot.last_seed, 0);

        // New player RNGs must be seeded from the shifted master as well.
        let mut registry = crate::player_manager::RandomRegistry {
            process: snapshot.process,
            head: super::HEAD,
            tail: super::TAIL,
        };
        let mut bytes = [0; 20];
        registry
            .initialize_rng(1, &mut bytes, &mut |tail, next| {
                assert_eq!((tail, next), (super::TAIL, 1));
                Ok(())
            })
            .unwrap();
        assert_eq!(crate::player_manager::word(&bytes, 0), 27254);
    }

    #[test]
    #[ignore = "requires original static registry reseed observations"]
    fn matches_original_static_registry() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_STATIC_RNG")).unwrap();
        assert_eq!(&bytes[..8], b"MTSTRNG2");
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        // MTSTRNG2 was produced by manually invoking 29 selected RNG
        // constructors, not the full CRT sequence. Its producer omitted the
        // static player's heap receiver. Preserve that narrower oracle by
        // comparing the same reseed implementation with this node removed.
        let legacy_references: [u32; 29] = REFERENCES
            .into_iter()
            .filter(|&reference| reference != STATIC_PLAYER_RECEIVER)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(word(12), legacy_references.len() as u32);
        let mut at = 16;
        for _ in 0..word(8) {
            let (states, process, last_seed) = super::reseed(word(at), legacy_references);
            at += 4;
            for (reference, state) in legacy_references.into_iter().zip(states) {
                assert_eq!(reference, word(at));
                at += 4;
                assert_eq!(state, word(at));
                at += 4;
            }
            for value in process {
                assert_eq!(value, word(at));
                at += 4;
            }
            assert_eq!(last_seed, word(at));
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }
}
