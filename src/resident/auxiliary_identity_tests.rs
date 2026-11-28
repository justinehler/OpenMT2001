//! Actual production constructor/allocation/replace/reset/drop regressions.
use super::*;
#[test]
fn allocated_bindings_are_stable_and_append_does_not_revoke() {
    let mut pool = AuxiliaryPool::new(3);
    assert!(pool.bind(0).is_err());
    assert_eq!(pool.allocate(&[0x5a; 284]), Some(0));
    let first = pool.bind(0).unwrap();
    assert_eq!(first, pool.bind(0).unwrap());
    assert_eq!(pool.allocate(&[0xa5; 284]), Some(1));
    assert_eq!(pool.resolve(&first).unwrap(), &[0x5a; 284]);
    assert!(pool.bind(2).is_err());
    assert!(pool.bind(3).is_err());
    pool.get_mut(0).unwrap()[0x30..0x32].copy_from_slice(&(-2i16).to_le_bytes());
    assert_eq!(
        i16::from_le_bytes(
            pool.resolve(&first).unwrap()[0x30..0x32]
                .try_into()
                .unwrap()
        ),
        -2
    );
}
#[test]
fn imported_equal_payload_has_distinct_origin_and_drop_revokes() {
    assert!(AuxiliaryPool::from_slots(vec![[0; 284]], 2).is_err());
    let pool = AuxiliaryPool::from_slots(vec![[7; 284]; 3], 2).unwrap();
    let binding = pool.bind(1).unwrap();
    let source = pool.source();
    let other = AuxiliaryPool::from_slots(pool.slots().to_vec(), pool.used()).unwrap();
    assert!(other.resolve(&binding).is_err());
    assert!(pool.bind(2).is_err());
    assert_ne!(binding, other.bind(1).unwrap());
    drop(pool);
    assert!(binding.validate_live().is_err());
    assert!(source.validate_live().is_err());
    assert!(other.bind(1).unwrap().validate_live().is_ok());
}
#[test]
fn real_replacement_and_reset_revoke_before_new_bindings() {
    let mut pool = AuxiliaryPool::from_slots(vec![[0xcc; 284]; 2], 1).unwrap();
    let first = pool.bind(0).unwrap();
    let source = pool.source();
    let equal = AuxiliaryPool::from_slots(pool.slots().to_vec(), pool.used()).unwrap();
    pool.replace_with(equal);
    assert!(first.validate_live().is_err());
    assert!(source.validate_live().is_err());
    let next = pool.bind(0).unwrap();
    assert_ne!(first, next);
    pool.reset_for_residents();
    assert!(next.validate_live().is_err());
    assert_eq!(pool.used(), 0);
    assert_eq!(pool.slots().len(), 2000);
    assert!(pool.bind(0).is_err());
}
#[test]
fn direct_assignment_drops_old_authority_and_exhaustion_preserves_live() {
    let mut pool = AuxiliaryPool::new(1);
    pool.allocate(&[3; 284]).unwrap();
    let old = pool.bind(0).unwrap();
    assert_eq!(pool.allocate(&[4; 284]), None);
    assert_eq!(pool.resolve(&old).unwrap(), &[3; 284]);
    pool = AuxiliaryPool::from_slots(vec![[3; 284]], 1).unwrap();
    assert!(old.validate_live().is_err());
    assert!(pool.resolve(&old).is_err());
}

// Fixture callback/model words remain their retained opaque/model seams; these
// tests do not claim whole-native behavior or native pointer identity.
#[test]
#[ignore = "requires supplied original creation fixture"]
fn retained_noncontiguous_creation_has_real_owned_auxiliary_identity() {
    check_retained_owned_creation(false);
}
#[test]
#[ignore = "requires supplied original load fixture"]
fn retained_saved777_is_not_aux_index_or_descriptor38() {
    check_retained_owned_creation(true);
}
fn check_retained_owned_creation(load: bool) {
    use crate::economy::{
        ResidentAccounts, ResidentCreationError, ResidentCreationInputs, ResidentCreationState,
        ResidentModelBinding,
    };
    let bytes = std::fs::read(crate::fixtures::path(if load {
        "OPENMT2001_NATIVE_RESIDENT_LOAD"
    } else {
        "OPENMT2001_NATIVE_RESIDENT_CREATION"
    }))
    .unwrap();
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!(&bytes[..8], if load { b"MTLOADR1" } else { b"MTCREATE" });
    assert_eq!(word(8), if load { 4 } else { 8 });
    let source = std::fs::read_to_string(crate::fixtures::path("DEFAULT_SETTINGS")).unwrap();
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = crate::pe::PeImage::parse(&exe).unwrap();
    let preferences = super::Preferences::from_v14_defaults(&source, &image).unwrap();
    let groups = super::PreferenceGroups::from_v14_defaults(&source).unwrap();
    let parameters = std::fs::read_to_string(crate::fixtures::path("PARAMETERS")).unwrap();
    let movement = super::MovementSettings::from_parameters(&parameters, &image).unwrap();
    assert_eq!(
        movement.speed_by_age.map(f32::to_bits),
        std::array::from_fn(|i| word(12 + i * 4))
    );
    assert_eq!(movement.variation.to_bits(), word(32));
    let saved_bytes = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
    let save = crate::save::SaveFile::parse(&saved_bytes).unwrap();
    let commodities = save.economy().unwrap().resident_commodities();
    let objectives = super::DailyObjectives {
        rows: std::array::from_fn(|i| (0..3).map(|j| word(36 + (i * 3 + j) * 4) as i32).collect()),
    };
    let timetables = super::Timetables {
        rows: std::array::from_fn(|i| std::array::from_fn(|j| word(108 + (i * 24 + j) * 4) as i32)),
    };
    let commodity_masks = std::array::from_fn(|i| word(9708 + i * 4));
    let models = std::array::from_fn(|i| ResidentModelBinding {
        reference: word(9728 + i * 120),
        definition: bytes[9732 + i * 120..9848 + i * 120].try_into().unwrap(),
    });
    let mut at = 14192;
    for case in 0..word(8) {
        let mut categories = std::array::from_fn(|i| crate::housing::Category {
            buildings: vec![],
            value: 0.,
            center: Some([
                f32::from_bits(word(13568 + i * 12)),
                f32::from_bits(word(13572 + i * 12)),
            ]),
            resident_counter: word(13576 + i * 12),
        });
        let requested = [word(at) as i32, word(at + 4) as i32, word(at + 8) as i32];
        let hour = word(at + 12) as usize;
        let initialization_rng = super::InitializationRng {
            constructor: word(at + 16),
            movement: word(at + 20),
            objective: word(at + 24),
        };
        let identity_rng = word(at + 28);
        let preference_rng = word(at + 32);
        let active_count = word(at + 36);
        let capacity = word(at + 40) as usize;
        let used = word(at + 44) as usize;
        let countdown_period = word(at + 48) as i32;
        at += 52;
        let nodes = bytes[at..at + 2000 * 0x150]
            .chunks_exact(0x150)
            .map(|r| {
                super::Resident::from_runtime_bytes(
                    r[..0x14a].try_into().unwrap(),
                    r[0x14a],
                    if load {
                        u32::from_le_bytes(r[0x14c..0x150].try_into().unwrap())
                    } else {
                        0
                    },
                )
            })
            .collect();
        at += 2000 * 0x150;
        let accounts = bytes[at..at + 2000 * 0x90]
            .chunks_exact(0x90)
            .map(|r| super::SpendingAccount::from_bytes(r.try_into().unwrap()))
            .collect();
        at += 2000 * 0x90;
        let slots = bytes[at..at + capacity * 0x11c]
            .chunks_exact(0x11c)
            .map(|r| r.try_into().unwrap())
            .collect();
        at += capacity * 0x11c;
        let template = bytes[at..at + 0x11c].try_into().unwrap();
        at += 0x11c;
        let mut residents = ResidentAccounts {
            nodes,
            accounts,
            active_count,
        };
        let mut state = ResidentCreationState {
            auxiliary: super::AuxiliaryPool::from_slots(slots, used).unwrap(),
            initialization_rng,
            identity_rng,
            preference_rng,
        };
        let old_source = state.auxiliary.source();
        let old_bindings: Vec<_> = (0..used)
            .map(|index| state.auxiliary.bind(index).unwrap())
            .collect();
        let personal = super::InitializationInputs {
            movement: &movement,
            movement_precision: super::MovementSpeedPrecision::AfterReset,
            preferences: &preferences,
            objectives: &objectives,
            timetables: &timetables,
            commodities: &commodities,
            commodity_masks,
            hour,
        };
        let inputs = ResidentCreationInputs {
            personal: &personal,
            preference_groups: &groups,
            models: &models,
            countdown_period,
        };
        let result = if load {
            let saved = bytes[at..at + 0x14a].try_into().unwrap();
            at += 0x14a;
            let centers = std::array::from_fn(|i| categories[i].center.unwrap());
            residents
                .load_resident(
                    saved,
                    template,
                    &inputs,
                    &mut state,
                    &mut categories,
                    &centers,
                )
                .map(Some)
        } else {
            residents.create_resident(requested, template, &inputs, &mut state, &mut categories)
        };
        old_source.validate_pool(&state.auxiliary).unwrap();
        for binding in &old_bindings {
            state.auxiliary.resolve(binding).unwrap();
        }
        let actual = match result {
            Ok(Some(index)) => (0, index as u32),
            Ok(None) => (1, u32::MAX),
            Err(ResidentCreationError::AuxiliaryExhausted { resident }) => (2, resident as u32),
            Err(ResidentCreationError::AccountsExhausted { resident }) => (3, resident as u32),
            Err(error) => panic!("case {case}: {error:?}"),
        };
        if !load && case == 1 {
            assert_eq!(actual, (0, 4));
            assert_eq!(state.auxiliary.used(), 2);
            let binding = state.auxiliary.bind(1).unwrap();
            assert_eq!(
                u32::from_le_bytes(
                    state.auxiliary.resolve(&binding).unwrap()[0x38..0x3c]
                        .try_into()
                        .unwrap()
                ),
                4
            );
        }
        if load && case == 1 {
            assert_eq!(actual, (0, 777));
            assert_eq!(residents.active_count, 2);
            assert_eq!(state.auxiliary.used(), 2);
            assert_eq!(residents.nodes[777].auxiliary_reference(), 1);
            let binding = state.auxiliary.bind(1).unwrap();
            assert_eq!(
                u32::from_le_bytes(
                    state.auxiliary.resolve(&binding).unwrap()[0x38..0x3c]
                        .try_into()
                        .unwrap()
                ),
                0
            );
        }
        assert_eq!(actual, (word(at), word(at + 4)), "case {case} outcome");
        assert_eq!(
            residents.active_count,
            word(at + 8),
            "case {case} active count"
        );
        assert_eq!(
            state.auxiliary.used(),
            word(at + 12) as usize,
            "case {case} aux used"
        );
        at += 16;
        assert_eq!(
            [
                state.initialization_rng.constructor,
                state.initialization_rng.movement,
                state.initialization_rng.objective,
                state.identity_rng,
                state.preference_rng
            ],
            std::array::from_fn(|i| word(at + i * 4)),
            "case {case} RNG"
        );
        at += 20;
        for (i, category) in categories.iter().enumerate() {
            assert_eq!(
                category.resident_counter,
                word(at + i * 4),
                "case {case} category {i}"
            );
        }
        at += 52 * 4;
        for (i, node) in residents.nodes.iter().enumerate() {
            let start = at + i * 0x150;
            if load {
                assert_eq!(
                    node.auxiliary_reference(),
                    word(start + 0x14c),
                    "case {case} node {i} auxiliary reference"
                );
            }
            for (offset, (&actual, &expected)) in node
                .as_bytes()
                .iter()
                .zip(&bytes[start..start + 0x14a])
                .enumerate()
            {
                assert_eq!(actual, expected, "case {case} node {i} offset {offset:x}");
            }
            assert_eq!(
                node.active_byte(),
                bytes[start + 0x14a],
                "case {case} node {i} active"
            );
        }
        at += 2000 * 0x150;
        for (i, account) in residents.accounts.iter().enumerate() {
            let start = at + i * 0x90;
            assert_eq!(
                account.as_bytes().as_slice(),
                &bytes[start..start + 0x90],
                "case {case} account {i}"
            );
        }
        at += 2000 * 0x90;
        for (i, slot) in state.auxiliary.slots().iter().enumerate() {
            let start = at + i * 0x11c;
            assert_eq!(
                slot.as_slice(),
                &bytes[start..start + 0x11c],
                "case {case} auxiliary {i}"
            );
        }
        at += capacity * 0x11c;
        let issued: Vec<_> = (0..state.auxiliary.used())
            .map(|i| state.auxiliary.bind(i).unwrap())
            .collect();
        state.auxiliary.reset_for_residents();
        assert!(old_source.validate_live().is_err());
        assert!(
            issued
                .iter()
                .all(|binding| binding.validate_live().is_err())
        );
    }
    assert_eq!(at, bytes.len());
}
