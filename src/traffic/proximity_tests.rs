//! Differential tests for the private original `0044ee90` proximity corpus.
//!
//! The parent module deliberately does not include this file in production;
//! the root integration can add it to its test module when the fixture is
//! available.

#[cfg(test)]
mod tests {
    use crate::traffic::scene::Scene;
    use crate::traffic::{HourlyEffects, RouteSound};

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

    fn f32_from_bits(bits: u32) -> f32 {
        f32::from_bits(bits)
    }

    fn apply_mutation(
        scene: &mut Scene,
        tick: &mut u32,
        camera: &mut [f32; 3],
        op: u32,
        args: [u32; 3],
    ) {
        match op {
            1 => *tick = args[0],
            2 => camera[args[0] as usize] = f32_from_bits(args[1]),
            3 => crate::traffic::put(&mut scene.aircraft[args[0] as usize], 0x10, args[1]),
            4 => crate::traffic::put(&mut scene.aircraft[args[0] as usize], 0, args[1]),
            5 => crate::traffic::put(&mut scene.trains[args[0] as usize], 0, args[1]),
            6 => crate::traffic::put(
                &mut scene.aircraft[args[0] as usize],
                0x18 + args[1] as usize * 4,
                f32_from_bits(args[2]).to_bits(),
            ),
            _ => panic!("unknown native proximity mutation opcode {op}"),
        }
    }

    #[test]
    #[ignore = "requires original0044ee90 proximity observations"]
    fn matches_native_transport_proximity() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_TRANSPORT_PROXIMITY")).unwrap();
        let mut reader = Reader(&bytes);
        assert_eq!(reader.take(8), b"MTPRX44\0");
        assert_eq!(reader.word(), 1);
        let cases = reader.word();
        for case in 0..cases {
            let name = String::from_utf8(reader.blob()).unwrap();
            let initial_tick = reader.word();
            let mutation_count = reader.word() as usize;
            let call_count = reader.word() as usize;
            assert_eq!(reader.word() as usize, 600);
            assert_eq!(reader.word() as usize, 8 * 0x130);
            let returned = reader.word() != 0;
            let fault = reader.word() != 0;
            let initial_camera = [
                f32_from_bits(reader.word()),
                f32_from_bits(reader.word()),
                f32_from_bits(reader.word()),
            ];
            let mutations: Vec<(u32, u32, [u32; 3])> = (0..mutation_count)
                .map(|_| {
                    (
                        reader.word(),
                        reader.word(),
                        [reader.word(), reader.word(), reader.word()],
                    )
                })
                .collect();
            let before_tick = reader.word();
            let before_camera = [
                f32_from_bits(reader.word()),
                f32_from_bits(reader.word()),
                f32_from_bits(reader.word()),
            ];
            let before_trains: [[u8; 60]; 10] =
                std::array::from_fn(|_| reader.take(60).try_into().unwrap());
            let before_aircraft: [[u8; 304]; 8] =
                std::array::from_fn(|_| reader.take(304).try_into().unwrap());
            assert_eq!(before_tick, initial_tick, "native case {case} {name} tick");
            assert_eq!(
                before_camera.map(f32::to_bits),
                initial_camera.map(f32::to_bits)
            );

            let after_tick = reader.word();
            let after_camera = [
                f32_from_bits(reader.word()),
                f32_from_bits(reader.word()),
                f32_from_bits(reader.word()),
            ];
            let after_trains: [[u8; 60]; 10] =
                std::array::from_fn(|_| reader.take(60).try_into().unwrap());
            let after_aircraft: [[u8; 304]; 8] =
                std::array::from_fn(|_| reader.take(304).try_into().unwrap());
            let expected_calls: Vec<[u32; 4]> = (0..call_count)
                .map(|_| {
                    assert_eq!(
                        reader.word(),
                        0x004aefc0,
                        "native case {case} {name} address"
                    );
                    let _ecx = reader.word();
                    [reader.word(), reader.word(), reader.word(), reader.word()]
                })
                .collect();

            let mut scene = Scene {
                hourly: HourlyEffects {
                    tier: 0,
                    random: [0; 3],
                    model_choice: 0,
                    variant: 0,
                    auxiliaries: [[0; 300]; 2],
                    effects: [[0; 20]; 9],
                    models: [0; 4],
                },
                aircraft: before_aircraft,
                trains: before_trains,
                environment: Default::default(),
                first_frame: false,
            };
            let mut tick = before_tick;
            let mut camera = before_camera;
            let mut actual_calls = Vec::<[u32; 4]>::new();
            scene
                .proximity_sounds(
                    &mut tick,
                    &mut camera,
                    |sound: RouteSound, scene, tick, camera| {
                        let event = actual_calls.len() as u32;
                        actual_calls.push([
                            sound.id,
                            sound.distance.to_bits(),
                            sound.arguments[0],
                            sound.arguments[1],
                        ]);
                        for &(index, op, args) in &mutations {
                            if index == event {
                                apply_mutation(scene, tick, camera, op, args);
                            }
                        }
                        Ok(())
                    },
                )
                .unwrap();
            assert!(
                returned && !fault,
                "native case {case} {name} did not return"
            );
            assert_eq!(actual_calls, expected_calls, "audio case {case} {name}");
            assert_eq!(tick, after_tick, "tick case {case} {name}");
            assert_eq!(camera.map(f32::to_bits), after_camera.map(f32::to_bits));
            for (index, row) in scene.trains.iter().enumerate() {
                assert_eq!(
                    row, &after_trains[index],
                    "train row {index} case {case} {name}"
                );
            }
            for (index, row) in scene.aircraft.iter().enumerate() {
                assert_eq!(
                    row, &after_aircraft[index],
                    "aircraft row {index} case {case} {name}"
                );
            }
        }
        assert!(reader.0.is_empty());
    }
}
