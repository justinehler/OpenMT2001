use super::*;
use openmt2001::resident::{AuxiliaryPool, Resident};

fn moving_resident(source: &Resident, reference: u32, complete: bool, moving: bool) -> Resident {
    let mut bytes = *source.as_bytes();
    bytes[0x7e] = (bytes[0x7e] & !5) | (u8::from(moving) * 4) | u8::from(!complete);
    bytes[0x10c] = u8::from(complete);
    for (at, value) in [
        (0xf4, if complete { 498_f32 } else { 480. }),
        (0xf8, 0.),
        (0xfc, 460.),
        (0x100, 500.),
        (0x104, 0.),
        (0x108, 460.),
        (0x110, 1.),
        (0x114, 0.),
        (0x118, 0.),
        (0x11c, if complete { 2. } else { 20. }),
        (0x120, 2.),
        (0x124, 500.),
        (0x128, 0.),
        (0x12c, 460.),
        (0x130, 500.),
        (0x134, 0.),
        (0x138, 460.),
    ] {
        put(&mut bytes, at, value.to_bits());
    }
    put(&mut bytes, 0xf0, reference);
    Resident::from_runtime_bytes(bytes, 1, reference)
}

fn encode_residents(
    residents: &openmt2001::economy::ResidentAccounts,
    auxiliary: &AuxiliaryPool,
) -> Vec<u8> {
    let mut output = Output(vec![]);
    output.word(residents.active_count);
    output.word(residents.nodes.len() as u32);
    for resident in &residents.nodes {
        output.0.extend_from_slice(resident.as_bytes());
        output.word(u32::from(resident.active_byte()));
        output.word(resident.auxiliary_reference());
    }
    output.word(auxiliary.used() as u32);
    output.blob(&flat(auxiliary.slots())).unwrap();
    output.0
}

fn assert_loaded_pose(resident: &Resident, actor: &[u8; 284], center: [f32; 2], complete: bool) {
    let x = if complete { 498_f32 } else { 482. };
    assert_eq!(word(resident.as_bytes(), 0xf4), x.to_bits());
    assert_eq!(
        word(resident.as_bytes(), 0x11c),
        if complete { 2_f32 } else { 18. }.to_bits()
    );
    assert_eq!(resident.as_bytes()[0x7e] & 4, if complete { 0 } else { 4 });
    assert_eq!(word(actor, 4), (x - 470.).to_bits());
    assert_eq!(word(actor, 8), 0.5_f32.to_bits());
    assert_eq!(word(actor, 12), 10_f32.to_bits());
    assert_eq!(word(actor, 0x14), 90_f32.to_bits());
    assert_eq!(
        word(resident.as_bytes(), 0xac),
        ((x - 470.) - center[0]).to_bits()
    );
    assert_eq!(word(resident.as_bytes(), 0xb0), word(actor, 8));
    assert_eq!(word(resident.as_bytes(), 0xb4), (10. - center[1]).to_bits());
    assert_eq!(actor[0xb8] & 1, !resident.as_bytes()[0x7e] & 1);
    assert_eq!(
        word(resident.as_bytes(), 0xf0),
        resident.auxiliary_reference()
    );
}

#[test]
fn resident_restore_keeps_native_fixups_and_actor_allocations() {
    let mut saved = openmt2001::economy::ResidentAccounts::new_pool();
    saved.active_count = 3;
    let mut old_actors = AuxiliaryPool::for_residents();
    for index in 0..5 {
        let mut row = [index as u8 + 0x30; 284];
        put(&mut row, 0, 100 + index as u32);
        old_actors.allocate(&row).unwrap();
    }
    old_actors.slots_mut()[1999] = [0xab; 284];
    let mut native = openmt2001::economy::ResidentAccounts::new_pool();
    native.active_count = 3;
    let mut actors = AuxiliaryPool::for_residents();
    let centers = [[7., 8.]; 52];
    let cases = [(3, false, true), (1, true, true), (5, false, false)];
    for (index, &(old_reference, complete, moving)) in cases.iter().enumerate() {
        saved.nodes[index] = moving_resident(&saved.nodes[index], old_reference, complete, moving);
        let slot = actors.allocate(&[0; 284]).unwrap();
        native.nodes[index] =
            Resident::from_runtime_bytes(*saved.nodes[index].as_bytes(), 1, slot as u32 + 1);
        native.nodes[index]
            .restore_saved_movement(slot as u32 + 1, actors.get_mut(slot).unwrap(), &centers)
            .unwrap();
    }
    let mut inactive = *saved.nodes[100].as_bytes();
    inactive[0x60] = 0xcd;
    saved.nodes[100] = Resident::from_runtime_bytes(inactive, 0, 2);
    let identity = actors.bind(0).unwrap();
    let old_identity = old_actors.bind(2).unwrap();
    let expected_nodes = native.nodes[..3]
        .iter()
        .map(|r| *r.as_bytes())
        .collect::<Vec<_>>();
    let references = restore_resident_runtime(
        &mut native,
        &mut actors,
        &encode_residents(&saved, &old_actors),
    )
    .unwrap();
    assert_eq!(references, [2, 4, 1, 5, 3]);
    assert!(
        actors.resolve(&identity).is_ok(),
        "extension must retain native allocations"
    );
    assert!(actors.resolve(&old_identity).is_err());
    for (index, &(_, complete, _)) in cases.iter().enumerate() {
        assert_eq!(*native.nodes[index].as_bytes(), expected_nodes[index]);
        let actor = &actors.slots()[index];
        assert_loaded_pose(&native.nodes[index], actor, centers[0], complete);
        assert_eq!(
            word(actor, 0),
            word(&old_actors.slots()[(cases[index].0 - 1) as usize], 0)
        );
        assert_eq!(
            &actor[0x2c..0x30],
            &old_actors.slots()[(cases[index].0 - 1) as usize][0x2c..0x30]
        );
    }
    assert_eq!(native.nodes[100].as_bytes()[0x60], 0xcd);
    assert_eq!(native.nodes[100].auxiliary_reference(), 4);
    assert_eq!(actors.slots()[3], old_actors.slots()[1]);
    assert_eq!(actors.slots()[4], old_actors.slots()[3]);
    assert_eq!(actors.slots()[1999], [0xab; 284]);
}

#[test]
#[ignore = "requires original v1.4 assets and scripts for engine Save/Load"]
fn engine_save_load_retains_resident_fixups_through_extension() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use crate::fresh_city::{Inputs, Policy};
            let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
            let exe = read("OPENMT2001_EXE");
            let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let build = read("OPENMT2001_BUILD_FILE");
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let index =
                openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let scripts = default.parent().unwrap().parent().unwrap();
            let styles = crate::fresh_city::load_era_styles(
                openmt2001::fixtures::path("PARAMETERS").parent().unwrap(),
            )
            .unwrap();
            let names = std::array::from_fn(|_| Vec::new());
            let inputs = || Inputs {
                tree_locators: None,
                image: &image,
                board: &board,
                file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
                scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: styles.clone(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario: "SCENARIO3",
                difficulty: 0,
                selected_character: 4,
                profile_name: b"Resident load",
                character_names: &names,
            };
            let policy = || Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 284],
            };
            let mut source = Session::initialize(inputs(), policy()).unwrap();
            assert!(source.world.ledgers.residents.active_count >= 3);
            // Saved allocation order need not be native reconstruction order.
            source.world.creation.auxiliary.slots_mut().swap(0, 2);
            for index in [0, 2] {
                let slot = 2 - index;
                let binding = &mut source.population.bindings[index];
                binding.auxiliary_slot = slot;
                binding.auxiliary_reference = slot as u32 + 1;
            }
            let cases = [(false, true), (true, true), (false, false)];
            for (index, &(complete, moving)) in cases.iter().enumerate() {
                let city = &mut source.world.city;
                let resident = &mut city.ledgers.residents.nodes[index];
                *resident = moving_resident(
                    resident,
                    source.population.bindings[index].auxiliary_reference,
                    complete,
                    moving,
                );
                let actor = city
                    .creation
                    .auxiliary
                    .get_mut((resident.auxiliary_reference() - 1) as usize)
                    .unwrap();
                put(actor, 4, 999_f32.to_bits());
                actor[0xb8] = (actor[0xb8] & !1) | (resident.as_bytes()[0x7e] & 1);
            }
            let binding = &source.population.bindings[0];
            let city = &mut source.world.city;
            city.ledgers.residents.nodes[0]
                .request_animation(
                    city.creation
                        .auxiliary
                        .get_mut(binding.auxiliary_slot)
                        .unwrap(),
                    &mut source.population.animations,
                    &source.population.animation_settings,
                    openmt2001::resident::ResidentAnimationRequest {
                        action: 2,
                        resident_reference: binding.resident_reference,
                        auxiliary_reference: binding.auxiliary_reference,
                        model_frame_count: binding.model_frame_count,
                    },
                )
                .unwrap();
            let camera = openmt2001::flyby::CameraSample {
                eye: [100., 120., 100.],
                target: [0.; 3],
                up: [0., 1., 0.],
                vertical_fov: 0.7,
            };
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            let bytes = source.save_v2(&image, cameras, 1).unwrap();
            let save = SaveFile::parse(&bytes).unwrap();
            assert_eq!(save.residents().unwrap().nodes[2][0x7e] & 4, 0);
            let mut loaded = Session::load_original(inputs(), policy(), &bytes).unwrap();
            let identity = loaded.world.creation.auxiliary.bind(0).unwrap();
            let native = loaded.world.ledgers.residents.nodes[..3]
                .iter()
                .map(|r| *r.as_bytes())
                .collect::<Vec<_>>();
            loaded
                .restore_v2_extension_with_image(&save, &image)
                .unwrap();
            loaded
                .repair_imported_resident_models(crate::compatibility::Mode::Modern, true)
                .unwrap();
            assert!(loaded.world.creation.auxiliary.resolve(&identity).is_ok());
            let centers = board.category_geometry().centers;
            for (index, &(complete, _)) in cases.iter().enumerate() {
                let resident = &loaded.world.ledgers.residents.nodes[index];
                assert_eq!(*resident.as_bytes(), native[index]);
                let binding = &loaded.population.bindings[index];
                assert_eq!(binding.auxiliary_reference, resident.auxiliary_reference());
                assert_eq!(
                    binding.auxiliary_slot as u32 + 1,
                    resident.auxiliary_reference()
                );
                let actor = &loaded.world.creation.auxiliary.slots()[binding.auxiliary_slot];
                assert_loaded_pose(
                    resident,
                    actor,
                    centers[resident.as_bytes()[0x7c] as usize],
                    complete,
                );
                let source_actor = &source.world.creation.auxiliary.slots()
                    [source.population.bindings[index].auxiliary_slot];
                assert_eq!(&actor[..4], &source_actor[..4], "retain saved model");
                assert_eq!(
                    &actor[0x2c..0x30],
                    &source_actor[0x2c..0x30],
                    "retain animation frame and controller handle"
                );
                if index == 0 {
                    assert_ne!(
                        binding.auxiliary_reference,
                        source.population.bindings[index].auxiliary_reference
                    );
                    let controller =
                        u16::from_le_bytes(actor[0x2e..0x30].try_into().unwrap()) as usize;
                    assert_eq!(
                        word(&loaded.population.animations.records()[controller], 8),
                        binding.auxiliary_reference
                    );
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
