//! Original44f0b0 dispatch and complete submitted auxiliary records.
use super::{models::Models, scene::Scene};
use crate::mesh::CachedMesh;

fn submitted_poses(state: &mut Scene, models: &Models<'_>, tick: u32) -> Vec<(u32, [f32; 3])> {
    let mut poses = Vec::new();
    state
        .advance_frame(tick, models, [0; 284], |auxiliary| {
            poses.push((
                super::word(auxiliary, 0),
                std::array::from_fn(|axis| f32::from_bits(super::word(auxiliary, 0xe0 + axis * 4))),
            ));
            Ok(())
        })
        .unwrap();
    poses
}

#[test]
fn fresh_transport_submits_water_poses_instead_of_locator_reference_frame() {
    // Minimal synthetic PE supplying the startup sequence tick and ship table.
    let mut executable = vec![0; 0x500];
    executable[..2].copy_from_slice(b"MZ");
    super::put(&mut executable, 0x3c, 0x40);
    executable[0x40..0x44].copy_from_slice(b"PE\0\0");
    executable[0x44..0x46].copy_from_slice(&0x14c_u16.to_le_bytes());
    executable[0x46..0x48].copy_from_slice(&1_u16.to_le_bytes());
    executable[0x54..0x56].copy_from_slice(&32_u16.to_le_bytes());
    executable[0x58..0x5a].copy_from_slice(&0x10b_u16.to_le_bytes());
    super::put(&mut executable, 0x74, 0x400000);
    super::put(&mut executable, 0x84, 0x139b00);
    super::put(&mut executable, 0x88, 0x300);
    super::put(&mut executable, 0x8c, 0x100);
    super::put(&mut executable, 0x17c, 1);
    let ships = [0x689970, 0x688b98, 0x6896e8, 0x687830];
    for (index, &reference) in ships.iter().enumerate() {
        super::put(&mut executable, 0x350 + index * 4, reference);
    }
    let image = crate::pe::PeImage::parse(&executable).unwrap();
    let header = [0; 164];
    // Frame zero is an exporter reference pose at the city origin. Only
    // animation frame one contains a world position on this synthetic river.
    let vertices: Vec<_> = [[0_f32; 3], [1200., -10., 700.]]
        .into_iter()
        .flat_map(|origin| {
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
                .map(|offset| std::array::from_fn::<_, 3, _>(|axis| origin[axis] + offset[axis]))
        })
        .flatten()
        .flat_map(f32::to_le_bytes)
        .collect();
    let references = (0..4)
        .flat_map(|i| [0x688db0 + i * 16, 0x687790 + i * 16])
        .chain(ships)
        .chain([0x687690, 0x6876a0]);
    let models = Models::from_fixture_entries(
        references
            .map(|reference| {
                (
                    reference,
                    CachedMesh {
                        header: &header,
                        vertex_count: 4,
                        frames: 2,
                        flags: 0,
                        vertex_bytes: &vertices,
                        uv_bytes: &[],
                        normal_bytes: &[],
                        opaque_records_35: &[],
                        opaque_records_28: &[],
                        parts: vec![],
                        texture_names: vec![],
                    },
                )
            })
            .collect(),
    )
    .unwrap();
    for (choice, ship) in ships.into_iter().enumerate() {
        let mut state = Scene::fresh(&image, [(choice as u32) << 16, 1, 0]).unwrap();
        let boats = submitted_poses(&mut state, &models, 1);
        assert_eq!(boats.len(), 4);
        for (index, (reference, position)) in boats.iter().enumerate() {
            assert_eq!(*reference, 0x688db0 + index as u32 * 16);
            assert_eq!(*position, [1200., -10., -700.]);
        }
        state
            .hourly
            .advance(
                &mut super::HourlyInputs {
                    era: 8,
                    hour: 12,
                    population: 0,
                    tick: 100,
                },
                |_, _, _| Ok(()),
            )
            .unwrap();
        for tick in [100, 200] {
            let poses = submitted_poses(&mut state, &models, tick);
            assert_eq!(poses.len(), 5);
            assert_eq!(poses[4], (ship, [1200., -10., -700.]));
        }
    }
}

#[test]
#[ignore = "requires original v1.4 executable and transport meshes"]
fn original_fresh_transport_keeps_harbour_boats_and_tanker_off_plaza() {
    let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
    let executable = read("EXE");
    let archive = read("ARCHIVE_BIN");
    let directory = read("ARCHIVE_DIR");
    let image = crate::pe::PeImage::parse(&executable).unwrap();
    let index = crate::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let models = Models::from_original(&image, &index, &archive).unwrap();
    let mut state = Scene::fresh(&image, [3 << 16, 1, 0]).unwrap();
    let boats = submitted_poses(&mut state, &models, 1);
    state
        .hourly
        .advance(
            &mut super::HourlyInputs {
                era: 0,
                hour: 12,
                population: 0,
                tick: 100,
            },
            |_, _, _| Ok(()),
        )
        .unwrap();
    let poses = submitted_poses(&mut state, &models, 100);
    eprintln!("transport submissions: {poses:?}");
    assert_eq!(boats.len(), 4);
    for (index, &(reference, position)) in boats.iter().enumerate() {
        assert_eq!(reference, 0x688db0 + index as u32 * 16);
        let matrix = models
            .locator_matrix(0x687790 + index as u32 * 16, 1)
            .unwrap();
        assert_eq!(position, matrix[9..]);
        assert!(position[0] > 190. && position[2] < -160.);
    }
    assert_eq!(&poses[..4], boats);
    let (reference, position) = poses[4];
    assert_eq!(reference, 0x687830); // tanker.tm, selected by the noon table.
    assert_eq!(position, models.locator_matrix(0x687690, 1).unwrap()[9..]);
    assert!(position[0] > 1600. && position[2] < -700.);
}

struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        a
    }
    fn word(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn blob(&mut self) -> &'a [u8] {
        let n = self.word() as usize;
        self.take(n)
    }
}
#[test]
#[ignore = "requires original complete transport frame fixture"]
fn matches_native_transport_frame() {
    let data = std::fs::read(crate::fixtures::path("NATIVE_TRANSPORT_FRAME")).unwrap();
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = crate::pe::PeImage::parse(&exe).unwrap();
    let mut reader = Reader(&data);
    assert_eq!(reader.take(8), b"MTFRAME1");
    assert_eq!(reader.word(), 1);
    let count = reader.word();
    let refs: Vec<_> = reader
        .blob()
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut submitted_models = std::collections::BTreeSet::new();
    for case in 0..count {
        let first = reader.word() != 0;
        let tick = reader.word();
        let frame = reader.word();
        let _occupied = reader.word();
        let afterflag = reader.word() != 0;
        let calls = reader.word();
        let vertices = reader.blob();
        let prior: [u8; 284] = reader.blob().try_into().unwrap();
        let before: [&[u8]; 3] = std::array::from_fn(|_| reader.blob());
        let after: [&[u8]; 3] = std::array::from_fn(|_| reader.blob());
        let expected: Vec<_> = (0..calls).map(|_| reader.blob()).collect();
        let header = [0; 164];
        let meshes = refs
            .iter()
            .map(|&reference| {
                (
                    reference,
                    CachedMesh {
                        header: &header,
                        vertex_count: 4,
                        frames: 7,
                        flags: 0,
                        vertex_bytes: vertices,
                        uv_bytes: &[],
                        normal_bytes: &[],
                        opaque_records_35: &[],
                        opaque_records_28: &[],
                        parts: vec![],
                        texture_names: vec![],
                    },
                )
            })
            .collect();
        let models = Models::from_fixture_entries(meshes).unwrap();
        let mut state = Scene::fresh(&image, [0; 3]).unwrap();
        state.first_frame = first;
        state.environment.frame = frame;
        for (row, bytes) in state.aircraft.iter_mut().zip(before[0].chunks_exact(304)) {
            row.copy_from_slice(bytes);
        }
        for (row, bytes) in state.trains.iter_mut().zip(before[1].chunks_exact(60)) {
            row.copy_from_slice(bytes);
        }
        for (row, bytes) in state
            .hourly
            .effects
            .iter_mut()
            .zip(before[2].chunks_exact(20))
        {
            row.copy_from_slice(bytes);
        }
        let mut actual = Vec::new();
        state
            .advance_frame(tick, &models, prior, |aux| {
                actual.push(aux.to_vec());
                Ok(())
            })
            .unwrap();
        submitted_models.extend(
            actual
                .iter()
                .map(|aux| u32::from_le_bytes(aux[..4].try_into().unwrap())),
        );
        assert_eq!(state.first_frame, afterflag, "flag case {case}");
        let observed: [Vec<u8>; 3] = [
            state.aircraft.iter().flatten().copied().collect(),
            state.trains.iter().flatten().copied().collect(),
            state.hourly.effects.iter().flatten().copied().collect(),
        ];
        for i in 0..3 {
            assert_eq!(observed[i], after[i], "pool{i} case{case}");
        }
        for (index, (a, b)) in actual.iter().zip(&expected).enumerate() {
            let diffs: Vec<_> = a
                .iter()
                .zip(b.iter())
                .enumerate()
                .filter(|(_, (x, y))| x != y)
                .map(|(i, _)| i)
                .collect();
            assert!(
                diffs.is_empty(),
                "case{case} submit{index} diff offsets {diffs:?}"
            );
        }
        assert_eq!(actual.len(), expected.len(), "submit count case{case}");
    }
    assert!(reader.0.is_empty());
    for i in 0..4 {
        let reference =
            u32::from_le_bytes(image.at(0x539d50 + i * 4, 4).unwrap().try_into().unwrap());
        assert!(
            submitted_models.contains(&reference),
            "native hourly ship {reference:08x} not covered"
        );
    }
}
