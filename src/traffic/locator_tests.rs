//! Original matrix and aircraft consumer comparisons, including whole records.
use super::{
    HourlyEffects, models,
    scene::{Environment, Scene},
};
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
#[ignore = "requires original transport matrix and aircraft fixture"]
fn native_locator_and_aircraft_match() {
    let data = std::fs::read(crate::fixtures::path("LOCATOR_FIXTURE")).unwrap();
    let mut reader = Reader(&data);
    assert_eq!(reader.take(8), b"MTLOCAT1");
    assert_eq!(reader.word(), 1);
    for case in 0..reader.word() {
        let frame = reader.word();
        let variant = reader.word();
        let _shape = reader.word();
        let aircraft = reader.word() != 0;
        let tick = reader.word();
        let count = reader.word();
        let vertices = reader.blob();
        let before = reader.blob();
        let expected = reader.blob();
        let calls: Vec<_> = (0..count).map(|_| reader.blob()).collect();
        let header = [0; 164];
        let mesh = CachedMesh {
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
        };
        if aircraft {
            let models =
                models::Models::from_locator_fixture(0x685cc8 + variant * 16, mesh).unwrap();
            let mut state = Scene {
                first_frame: false,
                hourly: HourlyEffects {
                    tier: 0,
                    random: [0; 3],
                    model_choice: 0,
                    variant: 0,
                    auxiliaries: [[0; 300]; 2],
                    effects: [[0; 20]; 9],
                    models: [0; 4],
                },
                aircraft: [[0; 304]; 8],
                trains: [[0; 60]; 10],
                environment: Environment {
                    crane_position: [0.; 3],
                    ship_position: [0.; 3],
                    frame: 0,
                    sequence_tick: 1,
                    sequence_active: 0,
                    frozen: 0,
                },
            };
            state.aircraft[0].copy_from_slice(before);
            let mut observed = Vec::new();
            state
                .aircraft_frame(0, tick, &models, |row| {
                    observed.push(row.to_vec());
                    Ok(())
                })
                .unwrap();
            assert_eq!(&state.aircraft[0][..], expected, "aircraft case {case}");
            assert_eq!(observed, calls, "submission case {case}");
        } else {
            let matrix = models::locator_matrix(&mesh, frame).unwrap();
            let bytes: Vec<_> = matrix.iter().flat_map(|v| v.to_le_bytes()).collect();
            assert_eq!(bytes, expected, "matrix case {case}");
            assert!(models::locator_matrix(&mesh, u32::MAX).is_err());
        }
    }
    assert!(reader.0.is_empty());
}
