//! Original464f60 auction update. Command routing remains synchronous.
use crate::player_manager::{put, word};

pub struct World<'a> {
    pub record: &'a mut [u8; 128],
    pub selected: &'a mut [u32; 2],
    pub active: &'a [u8],
    pub network: [bool; 2],
    pub command_prior: [[u8; 32]; 2],
    /// Original53c710 opening callback flag.
    pub opening: &'a mut u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Message([i32; 8]),
    Queue { descriptor: [u8; 32], size: usize },
}
fn local(world: &World<'_>) -> bool {
    !world.network[0] || world.network[1]
}
fn command(
    world: &World<'_>,
    prior: usize,
    kind: u32,
    delayed: bool,
    payload: [u32; 2],
) -> [u8; 32] {
    let mut result = world.command_prior[prior];
    put(
        &mut result,
        0,
        if kind == 0x20 { 0x525c68 } else { 0x525c58 },
    );
    put(&mut result, 4, kind);
    put(&mut result, 12, 0);
    result[16] = u8::from(delayed);
    if kind == 0x1f {
        put(&mut result, 20, payload[0]);
        put(&mut result, 24, payload[1]);
    }
    result
}
pub fn update(
    world: &mut World<'_>,
    mut call: impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    // Both source values are binary32; this sum and power-of-two scale produce
    // the same final binary32 rounding as original PC64 x87, including extremes.
    let average = (f64::from(f32::from_bits(world.selected[1]))
        + f64::from(f32::from_bits(world.selected[0])))
        * 0.5;
    world.selected[0] = (average as f32).to_bits();
    if local(world) {
        match word(world.record, 0x74) {
            1 => {
                for i in 0..world.active.len() {
                    call(
                        Call::Message([
                            i as i32,
                            -1,
                            0x27,
                            word(world.record, 4) as i32,
                            0,
                            0,
                            0,
                            -999,
                        ]),
                        world,
                    )?;
                }
                if local(world) {
                    let descriptor = command(world, 0, 0x1f, false, [4, 2]);
                    call(
                        Call::Queue {
                            descriptor,
                            size: 32,
                        },
                        world,
                    )?;
                }
            }
            2 => {
                if word(world.record, 0x60) as i32 > -1 {
                    put(world.record, 0x78, 0);
                    if local(world) {
                        let descriptor = command(world, 0, 0x1f, false, [4, 3]);
                        call(
                            Call::Queue {
                                descriptor,
                                size: 32,
                            },
                            world,
                        )?;
                    }
                } else if world.record[0x70] != 0 {
                    put(world.record, 0x78, 0);
                    if local(world) {
                        let descriptor = command(world, 0, 0x1f, false, [4, 4]);
                        call(
                            Call::Queue {
                                descriptor,
                                size: 32,
                            },
                            world,
                        )?;
                    }
                }
                let turn = word(world.record, 0x58) as usize;
                if *world
                    .active
                    .get(turn)
                    .ok_or("auction update turn outside roster")?
                    == 0
                {
                    let descriptor = command(world, 0, 0x1f, false, [2, turn as u32]);
                    call(
                        Call::Queue {
                            descriptor,
                            size: 32,
                        },
                        world,
                    )?;
                }
            }
            mode @ (3 | 4) => {
                let count = word(world.record, 0x78).wrapping_add(1);
                put(world.record, 0x78, count);
                if count as i32 > if mode == 3 { 150 } else { 100 } {
                    let descriptor = command(world, 0, 0x20, true, [0; 2]);
                    call(
                        Call::Queue {
                            descriptor,
                            size: 20,
                        },
                        world,
                    )?;
                    if local(world) {
                        let descriptor = command(world, 1, 0x1f, false, [4, 5]);
                        call(
                            Call::Queue {
                                descriptor,
                                size: 32,
                            },
                            world,
                        )?;
                    }
                }
            }
            _ => (),
        }
    }
    if word(world.record, 0x74) == 0 {
        // Both original4663d0 and4663f0 perform these identical stores and
        // invoke4645c0(1), the network-gated action4 producer.
        *world.opening = 1;
        if local(world) {
            let descriptor = command(world, 0, 0x1f, false, [4, 1]);
            call(
                Call::Queue {
                    descriptor,
                    size: 32,
                },
                world,
            )?;
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires original auction progress capture"]
fn matches_native_progress() {
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_PROGRESS").unwrap()).unwrap();
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
    }
    let mut r = Reader(&bytes);
    assert_eq!(r.take(8), b"MTAUPR01");
    assert_eq!(r.word(), 0x037f);
    let count = r.word();
    for case in 0..count {
        let network = r.take(2);
        let active = r.take(6);
        let mut record: [u8; 128] = r.take(128).try_into().unwrap();
        let mut selected = [r.word(), r.word()];
        let mut opening = r.take(1)[0];
        let expected = r.take(137);
        let n = r.word();
        let trace: Vec<Vec<u32>> = (0..n)
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut actual_trace = vec![];
        let mut world = World {
            record: &mut record,
            selected: &mut selected,
            active,
            network: [network[0] != 0, network[1] != 0],
            command_prior: [[0xcc; 32]; 2],
            opening: &mut opening,
        };
        update(&mut world, |call, _| {
            actual_trace.push(match call {
                Call::Message(args) => {
                    let mut v = vec![0];
                    v.extend(args.map(|v| v as u32));
                    v
                }
                Call::Queue { descriptor, size } => {
                    let mut v = vec![1, size as u32];
                    v.extend(
                        descriptor
                            .chunks_exact(4)
                            .map(|b| u32::from_le_bytes(b.try_into().unwrap())),
                    );
                    v
                }
            });
            Ok(())
        })
        .unwrap();
        // Native20-byte constructors leave the remaining12 stack bytes unrelated;
        // compare only their authoritative descriptor size.
        for (a, b) in actual_trace.iter_mut().zip(&trace) {
            if a[0] == 1 && a[1] == 20 {
                a[7..].copy_from_slice(&b[7..]);
            }
        }
        assert_eq!(actual_trace, trace, "case{case}");
        let mut actual = record.to_vec();
        for v in selected {
            actual.extend(v.to_le_bytes());
        }
        actual.push(opening);
        assert_eq!(actual, expected, "case{case}");
    }
    assert!(r.0.is_empty());
}
