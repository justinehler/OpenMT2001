// Native45bac0 reset, including prior script-created auxiliary members.
#[test]
#[ignore = "requires original resident pool reset fixture"]
fn matches_native_preload_resident_pool_reset() {
    let bytes =
        std::fs::read(openmt2001::fixtures::path("NATIVE_RESIDENT_POOL_RESET")).unwrap();
    let mut at = 0;
    let mut take = |n: usize| {
        let start = at;
        at += n;
        &bytes[start..at]
    };
    assert_eq!(take(8), b"MTRPRST1");
    assert_eq!(u32::from_le_bytes(take(4).try_into().unwrap()), 0x37f);
    let count = u32::from_le_bytes(take(4).try_into().unwrap());
    for _ in 0..count {
        let prior = u32::from_le_bytes(take(4).try_into().unwrap());
        let used_after = u32::from_le_bytes(take(4).try_into().unwrap());
        let active_after = u32::from_le_bytes(take(4).try_into().unwrap());
        let mut creation = openmt2001::startup_rng::Snapshot::from_seed(0x12345678)
            .resident_creation(openmt2001::resident::AuxiliaryPool::for_residents());
        for _ in 0..prior {
            creation.auxiliary.allocate(&[0x5a; 284]).unwrap();
        }
        let rng_before = [
            creation.initialization_rng.constructor,
            creation.initialization_rng.movement,
            creation.initialization_rng.objective,
            creation.identity_rng,
            creation.preference_rng,
        ];
        let mut residents = openmt2001::economy::ResidentAccounts::new_pool();
        residents.active_count = prior;
        for row in residents.nodes.iter_mut().take(prior as usize) {
            *row = openmt2001::resident::Resident::from_pool_bytes([0x5a; 330], 1);
        }
        super::reset_original_resident_pool(&mut creation, &mut residents);
        assert_eq!(creation.auxiliary.used(), used_after as usize);
        assert_eq!(creation.auxiliary.slots().len(), 2000);
        assert_eq!(residents.active_count, active_after);
        assert_eq!(
            [
                creation.initialization_rng.constructor,
                creation.initialization_rng.movement,
                creation.initialization_rng.objective,
                creation.identity_rng,
                creation.preference_rng
            ],
            rng_before
        );
        for node in &residents.nodes {
            assert_eq!(
                node.pool_index(),
                u16::from_le_bytes(take(2).try_into().unwrap())
            );
            assert_eq!(node.active_byte(), take(1)[0]);
        }
        let events = u32::from_le_bytes(take(4).try_into().unwrap());
        let calls: Vec<_> = (0..events)
            .map(|_| {
                (
                    u32::from_le_bytes(take(4).try_into().unwrap()),
                    u32::from_le_bytes(take(4).try_into().unwrap()),
                )
            })
            .collect();
        assert!(calls.iter().any(|call| call.0 == 0x45bac0));
        assert!(
            calls
                .iter()
                .any(|call| call.0 == 0x4dfad0 || call.1 == 568000)
        );
    }
    assert_eq!(at, bytes.len());
}
