//! Original44f0b0 dispatch and complete submitted auxiliary records.
use super::{models::Models, scene::Scene};
use crate::mesh::CachedMesh;
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
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_TRANSPORT_FRAME").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
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
        submitted_models.extend(actual.iter().map(|aux|
            u32::from_le_bytes(aux[..4].try_into().unwrap())));
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
        let reference = u32::from_le_bytes(image.at(0x539d50 + i * 4, 4)
            .unwrap().try_into().unwrap());
        assert!(submitted_models.contains(&reference), "native hourly ship {reference:08x} not covered");
    }
}
