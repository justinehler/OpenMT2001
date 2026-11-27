// Native load-stream fixture; no host file access in production admission.
use openmt2001::category_ownership::*;
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
#[test]
#[ignore = "requires original saved property transfer fixture"]
fn matches_native_saved_property_transfer_stream() {
    compare_native_saved_stream("OPENMT2001_NATIVE_SAVED_PROPERTY_TRANSFER", b"MTSTRN16");
}
#[test]
#[ignore = "requires original saved self-buyer transfer fixture"]
fn matches_native_saved_property_self_transfer_stream() {
    compare_native_saved_stream(
        "OPENMT2001_NATIVE_SAVED_PROPERTY_SELF_TRANSFER",
        b"MTSSLF16",
    );
}
#[test]
#[ignore = "requires original priced saved self-buyer transfer fixture"]
fn matches_native_saved_property_priced_self_transfer_stream() {
    compare_native_saved_stream(
        "OPENMT2001_NATIVE_SAVED_PROPERTY_PRICED_SELF_TRANSFER",
        b"MTSPAY16",
    );
}
fn compare_native_saved_stream(env: &str, magic: &[u8; 8]) {
    let bytes = std::fs::read(openmt2001::fixtures::path(env)).unwrap();
    let mut r = Reader::new(&bytes);
    assert_eq!(r.take(8), magic);
    assert_eq!(r.u32(), 0x037f);
    let count = r.u32();
    let executable = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    const SIZE: usize = 3 * 548 + 380 * 52 + 3 * 916 + 3 * 544;
    for case in 0..count {
        let config: [u32; 7] = std::array::from_fn(|_| r.u32());
        let mut before = Reader::new(r.take(SIZE));
        let expected = r.take(SIZE);
        let mut players: Vec<_> = (0..3)
            .map(|i| openmt2001::player_manager::Player {
                reference: 0x10010000 + i * 0x1000,
                bytes: before.take(548).to_vec(),
                auxiliaries: vec![],
            })
            .collect();
        let mut categories: [[u8; 380]; 52] = std::array::from_fn(|_| before.bytes());
        let mut buildings: Vec<[u8; 916]> = (0..3).map(|_| before.bytes()).collect();
        let mut businesses: Vec<[u8; 544]> = (0..3).map(|_| before.bytes()).collect();
        let traces = r.u32();
        let expected_trace: Vec<[u32; 3]> = (0..traces)
            .map(|_| std::array::from_fn(|_| r.u32()))
            .collect();
        let mut chains: [Vec<usize>; 52] = std::array::from_fn(|_| vec![]);
        if word(&categories[7], 0x15c) != 0 {
            chains[7] = vec![0, 1, 2];
        }
        let mut world = World {
            categories: &mut categories,
            buildings: &mut buildings,
            businesses: &mut businesses,
            players: &mut players,
            chains: &chains,
            normal_tick: 67890,
            category_tick: 12345,
            refresh_counter: config[4] as u8,
            accounting_enabled: config[6] != 0,
            current_player: Some(config[1] as usize),
            notification_threshold: 0.,
        };
        let mut trace = vec![];
        let mut callback = |world: &mut World<'_>, seam| {
            trace.push(match seam {
                Seam::NativeUiCall {
                    address: 0x4ad760,
                    args,
                    ..
                } => {
                    if config[3] != 0 {
                        world.categories[7][5] = 3;
                    }
                    [0, u32::from(args[0] == 0x541cc8), 0]
                }
                Seam::NativeUiCall {
                    address: address @ (0x4c6290 | 0x4c62d0),
                    ..
                } => [1, u32::from(address == 0x4c6290), 0],
                Seam::NativeUiCall {
                    address: 0x41f410, ..
                } => [7, 0, 0],
                Seam::AiOwnershipLists { player } => [6, world.players[player].reference, 0],
                other => panic!("unexpected transfer seam {other:?}"),
            });
            Ok(())
        };
        if case % 4 != 3 {
            if config[0] == 0 {
                world
                    .release(7, (config[2] as i32) as f32, &mut callback)
                    .unwrap();
            } else {
                world
                    .apply(
                        7,
                        config[0] as usize - 1,
                        (config[2] as i32) as f32,
                        &mut callback,
                    )
                    .unwrap();
            }
            let identities: Vec<_> = world.players.iter().map(|p| p.reference).collect();
            if config[0] != 0 {
                trace.push([8, config[2], 0]);
            }
            for player in world.players.iter_mut() {
                if player.bytes[0x10] == 1 {
                    update_monopoly(player, &identities, world.categories, &image).unwrap();
                }
            }
            trace.push([7, 0, 0]);
            property_view(&mut world.categories[7], 0, 0);
            trace.push([9, 7, 0]);
        } else {
            trace.push([10, 0, 1259]);
        }

        assert_eq!(u32::from(world.refresh_counter), config[5], "case {case}");
        assert_eq!(trace, expected_trace, "case {case}");
        let actual: Vec<u8> = world
            .players
            .iter()
            .flat_map(|p| p.bytes.iter().copied())
            .chain(world.categories.iter().flatten().copied())
            .chain(world.buildings.iter().flatten().copied())
            .chain(world.businesses.iter().flatten().copied())
            .collect();
        if let Some(at) = actual.iter().zip(expected).position(|(a, b)| a != b) {
            panic!(
                "case {case}, byte{at:x}: {} != {}",
                actual[at], expected[at]
            );
        }
    }
    assert_eq!(r.at, bytes.len());
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, size: usize) -> &'a [u8] {
        let start = self.at;
        self.at += size;
        &self.bytes[start..self.at]
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn bytes<const N: usize>(&mut self) -> [u8; N] {
        self.take(N).try_into().unwrap()
    }
}
