//! Source specifications over synthetic authored fixtures;
//! no model parsing, original numeric probe, game, renderer or F transaction.
use super::*;
use openmt2001::board::Block;
fn put(row: &mut [u8], at: usize, value: u32) {
    row[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn fixture() -> (
    crate::selection::Assets,
    [[u8; 380]; 52],
    [u8; 116],
    ResetTransform,
) {
    let mut rows = [[0; 380]; 52];
    let mut blocks = Vec::new();
    for i in 0..2 {
        let center = [10.0 + i as f32 * 100., 20.0 + i as f32 * 100.];
        let size: [f32; 2] = [30., 40.];
        put(&mut rows[i], 0, i as u32);
        for axis in 0..2 {
            put(&mut rows[i], 0xe0 + axis * 4, center[axis].to_bits());
            put(&mut rows[i], 0xd0 + axis * 4, size[axis].to_bits());
        }
        put(&mut rows[i], 0x94, 0xa0123456 + i as u32);
        blocks.push(Block {
            id: i,
            name: format!("authored{i}"),
            text_id: 700 + i as u32,
            kind: 0,
            center,
            size,
            pavement_color: 0xa0123456 + i as u32,
            flags: 0,
            street_sides: 0,
        });
    }
    let mut definition = [0; 116];
    for i in 0..29 {
        put(&mut definition, i * 4, (i as f32 + 0.25).to_bits());
    }
    let assets = crate::selection::Assets::descriptor_test_assets(blocks, definition);
    // Deliberately non-default matrix words detect hardcoded identity/zeros.
    (
        assets,
        rows,
        definition,
        ResetTransform(std::array::from_fn(|i| 0x10203000 + i as u32)),
    )
}
fn bind_model(assets: &crate::selection::Assets, definition: [u8; 116]) -> PavementModel {
    // Authored definition in a real finite owner, explicitly synthetic provenance.
    // No invented model71/reference words or production native handle values.
    assert_eq!(assets.pavement_definition().unwrap(), definition);
    PavementModel::bind(assets).unwrap()
}
fn selection() -> SelectionState {
    SelectionState::installed(
        None,
        None,
        None,
        RelatedKey::from_native_cell(0x702000),
        Some(0x703000),
        Some(9),
        12,
        13,
    )
}
use super::super::ui_selection::RelatedKey;

#[test]
fn constructor_writes_exact_fields_and_keeps_unwritten_bits() {
    let (_, _, _, reset) = fixture();
    let mut d = Descriptor::construct(&reset);
    for at in [0, 4, 8, 0xc, 0x10, 0x14, 0x18, 0x34, 0x38] {
        assert_eq!(d.word(at).unwrap(), 0);
    }
    for at in [0x1c, 0x20, 0x24] {
        assert_eq!(d.word(at).unwrap(), 0x3f800000);
    }
    for at in [0x28, 0x3c, 0x40] {
        assert_eq!(d.word(at).unwrap(), u32::MAX);
    }
    assert_eq!(d.word(0x2c).unwrap(), 0xffff0000);
    assert_eq!(d.kind().unwrap(), 0);
    assert!(d.word(0x30).is_err()); //kind known, upper+32 word NOT written
    for at in [0x32, 0x33, 0x44, 0xb9, 0xba, 0xbb] {
        assert_eq!(d.byte(at), Some(Bits::UNKNOWN));
    }
    assert_eq!(
        d.byte(0xb8),
        Some(Bits {
            value: 0x0d,
            known: 0x0f
        })
    );
    for i in 0..12 {
        assert_eq!(d.word(0xbc + i * 4).unwrap(), reset.0[i]);
        assert_eq!(d.word(0xec + i * 4).unwrap(), reset.0[i]);
    }
    //Reset is partial on a written preimage; flags high bits/+32/+b9 survive.
    d.bytes[0xb8] = Bits::exact(0xf2);
    d.bytes[0x32] = Bits::exact(0x57);
    d.bytes[0xb9] = Bits::exact(0x89);
    d.reset(&reset);
    assert_eq!(d.byte(0xb8), Some(Bits::exact(0xfd)));
    assert_eq!(d.byte(0x32), Some(Bits::exact(0x57)));
    assert_eq!(d.byte(0xb9), Some(Bits::exact(0x89)));
}
#[test]
fn producer_uses_live_rows_all_model_words_colors_and_active_flags() {
    let (assets, mut rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    rows[1][0x90] = 1; //Inactive still appended, not removed from native count.
    let pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    for i in 0..2 {
        let d = pool.resolve(pool.lookup(i).unwrap().unwrap()).unwrap();
        assert_eq!(d.word(0).unwrap(), 0x688b48);
        assert_eq!(d.kind().unwrap(), 4);
        assert_eq!(d.word(0x38).unwrap(), i as u32);
        assert_eq!(
            d.translation().unwrap(),
            [
                word(&rows[i as usize], 0xe0),
                0,
                word(&rows[i as usize], 0xe4)
            ]
        );
        for at in [0x3c, 0x40] {
            assert_eq!(d.word(at).unwrap(), word(&rows[i as usize], 0x94));
        }
        assert_eq!(d.byte(0xb8).unwrap().bit(1), Some(i == 0));
        for k in 0..29 {
            assert_eq!(d.word(0x44 + k * 4).unwrap(), word(&definition, k * 4));
        }
        for at in [0x32, 0x33, 0xb9, 0xba, 0xbb] {
            assert_eq!(d.byte(at), Some(Bits::UNKNOWN));
        }
    }
    assert!(pool.lookup(2).unwrap().is_none());
    assert!(pool.lookup(i32::MAX).unwrap().is_none());
    assert!(pool.lookup(-1).is_err());
    assert!(pool.lookup(i32::MIN).is_err());
}
#[test]
fn lookup_aliases_only_within_owned_current_allocation() {
    let (assets, rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    let mut pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    let first = pool.lookup(0).unwrap().unwrap();
    assert_eq!(first, pool.lookup(0).unwrap().unwrap());
    assert_ne!(first, pool.lookup(1).unwrap().unwrap());
    let other = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    assert!(other.resolve(first).is_err());
    pool.rebuild_rows(&rows, 2, &model, &reset).unwrap();
    assert_ne!(first, pool.lookup(0).unwrap().unwrap());
    assert!(pool.resolve(first).is_err());
    let current = pool.lookup(0).unwrap().unwrap();
    pool.retire_source_frame().unwrap();
    assert!(pool.resolve(current).is_err());
    assert!(pool.lookup(0).is_err());
    pool.rebuild_rows(&rows, 2, &model, &reset).unwrap();
    assert!(pool.resolve(current).is_err());
    let current = pool.lookup(0).unwrap().unwrap();
    assert!(pool.rebuild_rows(&rows, 53, &model, &reset).is_err());
    assert!(pool.resolve(current).is_err());
    assert!(pool.count().is_err());
    assert!(Pool::from_rows(&rows, -1, &model, &reset).is_err());
    let empty = Pool::from_rows(&rows, 0, &model, &reset).unwrap();
    assert!(empty.lookup(0).unwrap().is_none());
}
#[test]
fn fresh_lookup_publications_are_distinct_and_do_not_republish_or_clear_related() {
    let (assets, rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    let pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    let mut state = selection();
    state.published = Some(pool.selected(pool.lookup(1).unwrap().unwrap()).unwrap());
    let published = state.published;
    //A supported reentrant selector change between the two actual calls.
    let mut selected = 0;
    pool.lookup_publish_current(&mut state, selected).unwrap();
    selected = 1;
    pool.lookup_publish_transient(&mut state, selected).unwrap();
    assert_ne!(state.current, state.transient);
    assert_eq!(state.current.unwrap().category, 0);
    assert_eq!(state.transient.unwrap().category, 1);
    assert_eq!(state.published, published);
    assert_eq!(state.related.unwrap().native_key(), 0x702000);
    assert_eq!(state.selected_building, Some(0x703000));
    assert_eq!(
        (
            state.sequence_index,
            state.list_business,
            state.list_commodity
        ),
        (Some(9), 12, 13)
    );
    assert!(state.current.unwrap().key.native_key().is_err());
    pool.lookup_publish_transient(&mut state, 2).unwrap();
    assert!(state.transient.is_none());
    assert!(state.current.is_some());
}
#[test]
fn camera_uses_post_mode_current_and_preserves_separate_words() {
    let (assets, rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    let pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    let mut state = selection();
    pool.lookup_publish_current(&mut state, 0).unwrap();
    pool.lookup_publish_transient(&mut state, 0).unwrap();
    let retained = Some(pool.selected(pool.lookup(1).unwrap().unwrap()).unwrap());
    state.strategic_hide(retained); //Actual mode0's C-hide may restore this.
    let input = pool
        .camera_rows(
            state.current,
            &rows,
            2,
            &model,
            &assets,
            120f32.to_bits(),
            80f32.to_bits(),
        )
        .unwrap();
    assert_eq!(
        input.actor_translation,
        [word(&rows[1], 0xe0), 0, word(&rows[1], 0xe4)]
    );
    assert_eq!(
        input.model_offsets,
        [
            word(&definition, 24 * 4),
            word(&definition, 25 * 4),
            word(&definition, 26 * 4)
        ]
    );
    assert_eq!(state.transient.unwrap().category, 0);
    assert_eq!(state.current.unwrap().category, 1);
    assert_eq!(input.height_word, 120f32.to_bits());
    assert_eq!(input.radius_word, 80f32.to_bits());
    state.current = None;
    assert!(
        pool.camera_rows(state.current, &rows, 2, &model, &assets, 0, 0)
            .is_err()
    );
}
#[test]
fn camera_rejects_replaced_model_geometry_owner_and_stale_or_forged_current() {
    let (assets, mut rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    let mut pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    let mut s = selection();
    pool.lookup_publish_current(&mut s, 0).unwrap();
    let camera = |p: &Pool,
                  rows: &[[u8; 380]; 52],
                  m: &PavementModel,
                  a: &crate::selection::Assets,
                  current| p.camera_rows(current, rows, 2, m, a, 0, 0);
    let rebound = bind_model(&assets, definition);
    // Merely resolving the SAME installed generation must not mint identity.
    assert!(camera(&pool, &rows, &rebound, &assets, s.current).is_ok());
    put(&mut rows[0], 0xe0, 99f32.to_bits());
    assert!(camera(&pool, &rows, &model, &assets, s.current).is_err());
    put(&mut rows[0], 0xe0, 10f32.to_bits());
    put(&mut rows[0], 0xd0, 31f32.to_bits());
    assert!(camera(&pool, &rows, &model, &assets, s.current).is_err());
    put(&mut rows[0], 0xd0, 30f32.to_bits());
    let (other_assets, _, _, _) = fixture();
    assert!(camera(&pool, &rows, &model, &other_assets, s.current).is_err());
    let foreign = bind_model(&other_assets, definition);
    assert!(camera(&pool, &rows, &foreign, &assets, s.current).is_err());
    let mut forged = s.current.unwrap();
    forged.category = 1;
    assert!(camera(&pool, &rows, &model, &assets, Some(forged)).is_err());
    forged = s.current.unwrap();
    forged.key = SelectionKey::NativeObject(0x600000);
    assert!(camera(&pool, &rows, &model, &assets, Some(forged)).is_err());
    //Same model identity with corrupted cached model data must also reject.
    pool.records.as_mut().unwrap()[0].put_word(0x44 + 24 * 4, 0x12345678);
    assert!(camera(&pool, &rows, &model, &assets, s.current).is_err());
    pool.rebuild_rows(&rows, 2, &model, &reset).unwrap();
    assert!(camera(&pool, &rows, &model, &assets, s.current).is_err());
}
#[test]
fn root_retains_one_pool_and_resolves_each_reached_selection_read() {
    use super::super::owner::Owner;
    use openmt2001::camera::bank::{Bank, Capture};
    let (assets, rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    let pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    let mut owner = Owner::new(Bank::from_capture(Capture::from_records([[0; 196]; 2])), 0);
    owner.install_ui_selection(selection()).unwrap();
    owner.install_property_descriptors(pool).unwrap();
    owner.property_lookup_current(0).unwrap();
    owner.property_lookup_transient(1).unwrap();
    owner.ui_selection_mut().unwrap().publish_current();
    let current = owner.current_ui_selected().unwrap();
    assert_eq!(current, owner.published_ui_selected().unwrap());
    assert_ne!(current, owner.transient_ui_selected().unwrap());
    let duplicate = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    assert!(owner.install_property_descriptors(duplicate).is_err());
    owner
        .property_descriptors_mut()
        .unwrap()
        .rebuild_rows(&rows, 2, &model, &reset)
        .unwrap();
    assert!(owner.current_ui_selected().is_err());
    assert!(owner.published_ui_selected().is_err());
    assert!(owner.transient_ui_selected().is_err());
    owner.property_lookup_current(0).unwrap();
    assert_ne!(current, owner.current_ui_selected().unwrap());
    //Replacing current does not silently repair/overwrite another native cell.
    assert!(owner.published_ui_selected().is_err());
    assert!(owner.transient_ui_selected().is_err());
}

#[test]
fn camera_resource_replacement_revokes_equal_definition_and_requires_pool_rebuild() {
    let (mut assets, rows, definition, reset) = fixture();
    let old = PavementModel::bind(&assets).unwrap();
    let mut pool = Pool::from_rows(&rows, 2, &old, &reset).unwrap();
    let mut state = selection();
    pool.lookup_publish_current(&mut state, 0).unwrap();
    let stale = state.current;
    let before = pool
        .camera_rows(
            state.current,
            &rows,
            2,
            &old,
            &assets,
            0x12345678,
            0x87654321,
        )
        .unwrap();
    // Explicit test-only real directory replacement, not a second bind call.
    // The production caller must retire Pool first; even if it forgets,
    // source occupancy checks must revoke all reads of this old allocation.
    assets.replace_descriptor_test_resource(definition).unwrap();
    assert!(old.binding.definition().is_err());
    assert!(pool.resolve_selected(stale.unwrap()).is_err());
    let fresh = PavementModel::bind(&assets).unwrap();
    assert_ne!(old.binding.handle(), fresh.binding.handle());
    assert!(
        pool.camera_rows(state.current, &rows, 2, &fresh, &assets, 0, 0)
            .is_err()
    );
    assert!(pool.rebuild_rows(&rows, 2, &old, &reset).is_err());
    assert!(pool.count().is_err());
    pool.rebuild_rows(&rows, 2, &fresh, &reset).unwrap();
    assert!(pool.resolve_selected(stale.unwrap()).is_err());
    pool.lookup_publish_current(&mut state, 0).unwrap();
    let after = pool
        .camera_rows(
            state.current,
            &rows,
            2,
            &fresh,
            &assets,
            0x12345678,
            0x87654321,
        )
        .unwrap();
    assert_eq!(before.actor_translation, after.actor_translation);
    assert_eq!(before.model_offsets, after.model_offsets);
    assert_eq!(
        (before.height_word, before.radius_word),
        (after.height_word, after.radius_word)
    );
    // A different installed definition is used only after the actual rebuild.
    let mut changed = definition;
    put(&mut changed, 24 * 4, (-0.0f32).to_bits());
    pool.retire_source_frame().unwrap();
    assets.replace_descriptor_test_resource(changed).unwrap();
    let changed_model = PavementModel::bind(&assets).unwrap();
    pool.rebuild_rows(&rows, 2, &changed_model, &reset).unwrap();
    pool.lookup_publish_current(&mut state, 0).unwrap();
    let after = pool
        .camera_rows(state.current, &rows, 2, &changed_model, &assets, 0, 0)
        .unwrap();
    assert_eq!(after.model_offsets[0], (-0.0f32).to_bits());
    assert_eq!(after.actor_translation, before.actor_translation);
}
#[test]
fn resource_removal_and_owner_drop_revoke_pool_aliases_and_future_binds() {
    let (mut assets, rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    let pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    let handle = pool.lookup(0).unwrap().unwrap();
    assets.retire_pavement_resource().unwrap();
    assert!(PavementModel::bind(&assets).is_err());
    assert!(model.binding.definition().is_err());
    assert!(pool.resolve(handle).is_err());
    assert!(pool.lookup(0).is_err());
    let (assets, rows, definition, reset) = fixture();
    let model = bind_model(&assets, definition);
    let pool = Pool::from_rows(&rows, 2, &model, &reset).unwrap();
    let handle = pool.lookup(0).unwrap().unwrap();
    drop(assets);
    assert!(model.binding.definition().is_err());
    assert!(pool.resolve(handle).is_err());
    assert!(Pool::from_rows(&rows, 2, &model, &reset).is_err());
}

/// Direct World-writer regression. Actual World::fresh mints each
/// production-domain epoch; model occupancy is the explicit authored fixture.
/// Requires read-only supplied PE/parameters/timetable, never runs original code.
#[test]
#[ignore = "requires OPENMT2001_EXE, OPENMT2001_PARAMETERS and OPENMT2001_TIMETABLE; direct World rebuild lifecycle"]
fn world_rebuild_requires_preexisting_active_source_for_same_epoch() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let bytes = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
            let image = PeImage::parse(&bytes).unwrap();
            let parameters =
                std::fs::read_to_string(std::env::var("OPENMT2001_PARAMETERS").unwrap()).unwrap();
            let timetable =
                std::fs::read_to_string(std::env::var("OPENMT2001_TIMETABLE").unwrap()).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let make_world = || {
                let configuration = openmt2001::resident::Configuration::before_defaults(
                    &parameters,
                    &timetable,
                    &image,
                )
                .unwrap();
                let creation = openmt2001::startup_rng::Snapshot::from_seed(1)
                    .resident_creation(openmt2001::resident::AuxiliaryPool::for_residents());
                World::fresh(&image, &board, configuration, creation).unwrap()
            };
            let earlier = make_world();
            let active = make_world();
            let (assets, _, definition, reset) = fixture();
            let model = bind_model(&assets, definition);
            let mut pool = Pool::build(&active, &model, &reset).unwrap();
            let identity = pool.test_identity();
            let old = pool.lookup(0).unwrap().unwrap();
            pool.rebuild(&active, &model, &reset).unwrap();
            assert_eq!(pool.test_identity().0, identity.0);
            assert!(pool.resolve(old).is_err());
            let replaced = pool.lookup(0).unwrap().unwrap();
            pool.retire_source_frame().unwrap();
            for world in [&active, &earlier] {
                assert!(
                    pool.rebuild(world, &model, &reset)
                        .unwrap_err()
                        .contains("epoch")
                );
                assert!(pool.test_is_retired());
                assert!(pool.lookup(0).is_err());
                assert!(pool.resolve(replaced).is_err());
            }
            let next = make_world();
            pool.rebuild(&next, &model, &reset).unwrap();
            assert_eq!(pool.test_identity().0, identity.0);
            assert_eq!(pool.count().unwrap(), 52);
            // Same-World replacement is legal while active, but its failed attempt
            // retires that authority. Equality cannot restore it on the next call.
            let previous = pool.lookup(0).unwrap().unwrap();
            pool.test_fail_allocation();
            assert!(
                pool.rebuild(&next, &model, &reset)
                    .unwrap_err()
                    .contains("allocation failed")
            );
            assert!(pool.test_is_retired());
            assert!(pool.resolve(previous).is_err());
            assert!(
                pool.rebuild(&next, &model, &reset)
                    .unwrap_err()
                    .contains("epoch")
            );
            assert!(pool.test_is_retired());
            assert!(pool.lookup(0).is_err());
            let newer = make_world();
            pool.test_fail_allocation();
            assert!(
                pool.rebuild(&newer, &model, &reset)
                    .unwrap_err()
                    .contains("allocation failed")
            );
            assert!(
                pool.rebuild(&newer, &model, &reset)
                    .unwrap_err()
                    .contains("epoch")
            );
            assert!(pool.test_is_retired());
            let final_world = make_world();
            pool.rebuild(&final_world, &model, &reset).unwrap();
            assert_eq!(pool.test_identity().0, identity.0);
            assert_eq!(pool.count().unwrap(), 52);
            assert!(pool.resolve(old).is_err());
            assert!(pool.resolve(previous).is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
