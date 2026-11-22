//! Differential tests for the original `0044ec50` environment prefix.

#[cfg(test)]
mod tests {
    use crate::mesh::CachedMesh;
    use crate::traffic::HourlyEffects;
    use crate::traffic::models::Models;
    use crate::traffic::scene::{Environment, EnvironmentCall, Scene};

    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, size: usize) -> &'a [u8] {
            let (head, tail) = self.0.split_at(size);
            self.0 = tail;
            head
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> Vec<u8> {
            let size = self.word() as usize;
            self.take(size).to_vec()
        }
    }

    fn f32_from_bits(value: u32) -> f32 {
        f32::from_bits(value)
    }

    fn synthetic_models<'a>(header: &'a [u8; 164], vertices: &'a [u8]) -> Models<'a> {
        let mesh = CachedMesh {
            header,
            vertex_count: 1,
            frames: 4,
            flags: 0,
            vertex_bytes: vertices,
            uv_bytes: &[],
            normal_bytes: &[],
            opaque_records_35: &[],
            opaque_records_28: &[],
            parts: vec![],
            texture_names: vec![],
        };
        Models::from_locator_fixture(0x6883b0, mesh).unwrap()
    }

    fn apply_mutation(scene: &mut Scene, mutation: (u32, u32, [u32; 3])) {
        let (_, op, args) = mutation;
        match op {
            1 => scene.environment.sequence_tick = args[0],
            2 => scene.environment.sequence_tick = args[0],
            3 => scene.environment.frame = args[0],
            4 => scene.hourly.model_choice = args[0],
            5 => scene.environment.sequence_active = args[0] as u8,
            6 => scene.environment.frozen = args[0] as u8,
            7 => crate::traffic::put(&mut scene.hourly.effects[0], args[0] as usize, args[1]),
            _ => panic!("unknown environment mutation opcode {op}"),
        }
    }

    #[test]
    #[ignore = "requires original0044ec50 environment observations"]
    fn matches_native_transport_environment() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_TRANSPORT_ENVIRONMENT").unwrap())
            .unwrap();
        let mut reader = Reader(&data);
        assert_eq!(reader.take(8), b"MTENV44\0");
        assert_eq!(reader.word(), 1);
        let cases = reader.word();

        let header = [0_u8; 164];
        let points = [
            [3.0_f32, -4.0, -5.0],
            [1.0, 2.0, -3.0],
            [5.0, -10.0, -7.0],
            [3.0, -4.0, -5.0],
        ];
        let vertices: Vec<u8> = points
            .iter()
            .flat_map(|point| point.iter().flat_map(|value| value.to_le_bytes()))
            .collect();
        let models = synthetic_models(&header, &vertices);

        for case in 0..cases {
            let name = String::from_utf8(reader.blob()).unwrap();
            let mutation_count = reader.word() as usize;
            let call_count = reader.word() as usize;
            let returned = reader.word() != 0;
            let fault = reader.word() != 0;
            let _mode = reader.word();
            let mutations: Vec<(u32, u32, [u32; 3])> = (0..mutation_count)
                .map(|_| {
                    (
                        reader.word(),
                        reader.word(),
                        [reader.word(), reader.word(), reader.word()],
                    )
                })
                .collect();

            let read_state = |reader: &mut Reader<'_>| {
                let tick = reader.word();
                let model_choice = reader.word();
                let frame = reader.word();
                let sequence_active = reader.word() as u8;
                let frozen = reader.word() as u8;
                let sequence_tick = reader.word();
                let effect_bytes = reader.word() as usize;
                let crane: [f32; 3] = std::array::from_fn(|_| f32_from_bits(reader.word()));
                let ship: [f32; 3] = std::array::from_fn(|_| f32_from_bits(reader.word()));
                let effects = reader.take(effect_bytes).to_vec();
                (
                    tick,
                    model_choice,
                    frame,
                    sequence_active,
                    frozen,
                    sequence_tick,
                    crane,
                    ship,
                    effects,
                )
            };
            let before = read_state(&mut reader);
            let expected = read_state(&mut reader);
            let expected_calls: Vec<(u32, Vec<u32>)> = (0..call_count)
                .map(|_| {
                    let address = reader.word();
                    let _ecx = reader.word();
                    (
                        address,
                        vec![reader.word(), reader.word(), reader.word(), reader.word()],
                    )
                })
                .collect();

            let mut scene = Scene {
                hourly: HourlyEffects {
                    tier: 0,
                    random: [0; 3],
                    model_choice: before.1,
                    variant: 0,
                    auxiliaries: [[0; 300]; 2],
                    effects: [[0; 20]; 9],
                    models: [0; 4],
                },
                aircraft: [[0; 304]; 8],
                trains: [[0; 60]; 10],
                environment: Environment {
                    crane_position: before.6,
                    ship_position: before.7,
                    frame: before.2,
                    sequence_tick: before.5,
                    sequence_active: before.3,
                    frozen: before.4,
                },
                first_frame: false,
            };
            for (index, chunk) in before.8.chunks_exact(20).enumerate() {
                scene.hourly.effects[index].copy_from_slice(chunk);
            }
            let mut calls = Vec::<(u32, Vec<u32>)>::new();
            let mut tick = before.0;
            scene
                .environment_tick(tick, &models, |call, scene| {
                    let args = match call {
                        EnvironmentCall::Ship(value) => vec![value, 0, 0, 0],
                        EnvironmentCall::Cue(value) => vec![value, 0, 0, 0],
                        EnvironmentCall::Refresh => vec![0, 0, 0, 0],
                    };
                    calls.push((
                        match call {
                            EnvironmentCall::Ship(_) => 0x004addd0,
                            EnvironmentCall::Cue(_) => 0x004ae2b0,
                            EnvironmentCall::Refresh => 0x004ae1f0,
                        },
                        args,
                    ));
                    let event = (calls.len() - 1) as u32;
                    for &mutation in &mutations {
                        if mutation.0 == event {
                            apply_mutation(scene, mutation);
                        }
                    }
                    tick = tick.wrapping_add(0);
                    Ok(())
                })
                .unwrap();
            assert!(
                returned && !fault,
                "native case {case} {name} did not stop cleanly"
            );
            assert_eq!(calls, expected_calls, "call trace case {case} {name}");
            assert_eq!(
                scene.environment.crane_position.map(f32::to_bits),
                expected.6.map(f32::to_bits)
            );
            assert_eq!(
                scene.environment.ship_position.map(f32::to_bits),
                expected.7.map(f32::to_bits)
            );
            assert_eq!(
                scene.environment.frame, expected.2,
                "frame case {case} {name}"
            );
            assert_eq!(
                scene.environment.sequence_tick, expected.5,
                "sequence case {case} {name}"
            );
            assert_eq!(scene.environment.sequence_active, expected.3);
            assert_eq!(scene.environment.frozen, expected.4);
            assert_eq!(scene.hourly.model_choice, expected.1);
            let actual_effects: Vec<u8> = scene.hourly.effects.iter().flatten().copied().collect();
            assert_eq!(actual_effects, expected.8, "effects case {case} {name}");
        }
        assert!(reader.0.is_empty());
    }
}
