// Native command13 stream world and exact current-owner callback effects.
use openmt2001::business_sale;
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let start = self.at;
        self.at += n;
        &self.bytes[start..self.at]
    }
    fn word(&mut self) -> u32 {
        word(self.take(4), 0)
    }
}
fn rows<const N: usize>(bytes: &[u8]) -> Vec<[u8; N]> {
    bytes
        .chunks_exact(N)
        .map(|row| row.try_into().unwrap())
        .collect()
}
#[test]
#[ignore = "requires original saved business-sale stream fixture"]
fn matches_native_saved_business_sale_world_effects() {
    compare_sale_fixture("OPENMT2001_NATIVE_SAVED_BUSINESS_SALE", b"MTSLSAL2");
}
#[test]
#[ignore = "requires original sale with actual zero-entry appearance continuation"]
fn matches_native_saved_sale_zero_appearance() {
    compare_sale_fixture("OPENMT2001_NATIVE_SAVED_SALE_ZERO_APPEARANCE", b"MTSLZAP1");
}
fn compare_sale_fixture(variable: &str, magic: &[u8; 8]) {
    let bytes =
        std::fs::read(openmt2001::fixtures::path(variable)).unwrap();
    let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
    let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
    let mut r = Reader {
        bytes: &bytes,
        at: 0,
    };
    assert_eq!(r.take(8), magic);
    assert_eq!(r.word(), 0x37f);
    let count = r.word();
    let mut cash_cases = 0;
    for case in 0..count {
        let h: [u32; 6] = std::array::from_fn(|_| r.word());
        assert_eq!(h[4], 8);
        let regions: Vec<_> = (0..h[4])
            .map(|_| {
                let n = r.word() as usize;
                (r.take(n), r.take(n))
            })
            .collect();
        let mut expected_trace = vec![];
        for _ in 0..r.word() {
            let address = r.word();
            let argc = r.word();
            expected_trace.push((address, (0..argc).map(|_| r.word()).collect::<Vec<_>>()));
        }
        let mut businesses = rows::<544>(regions[0].0);
        let mut buildings = rows::<916>(regions[1].0);
        let catalog = openmt2001::business_pool::BusinessCatalog::from_rows(
            rows::<116>(regions[2].0).try_into().unwrap(),
        );
        let products = std::array::from_fn(|k| {
            std::array::from_fn(|i| {
                [
                    word(regions[3].0, k * 48 + i * 8),
                    word(regions[3].0, k * 48 + i * 8 + 4),
                ]
            })
        });
        let commodities = rows::<28>(regions[4].0);
        let mut players: Vec<_> = (5..8)
            .map(|i| openmt2001::economy::PlayerAccount::from_runtime(regions[i].0).unwrap())
            .collect();
        let mut age = (h[0] * 17) as u8;
        let mut actual_trace = vec![];
        if h[1] != 3 {
            let mut world = business_sale::World {
                businesses: &mut businesses,
                buildings: &mut buildings,
                players: &mut players,
                catalog: &catalog,
                products: &products,
                commodities: commodities.as_slice().try_into().unwrap(),
                age: &mut age,
            };
            business_sale::apply(
                &business_sale::Command {
                    business: 0,
                    player: (h[0] % 6) as u8,
                },
                &mut world,
                &business_sale::Controls {
                    current_player: if h[5] == 0 {
                        None
                    } else {
                        Some(((h[5] - 0x10030000) / 0x400) as usize)
                    },
                    accounting: false,
                    ownership_tick: 271 + h[0],
                    timestamp: 1200 + h[0],
                    hour: h[3] as i32,
                },
                &image,
                |call, _| {
                    actual_trace.push(match call {
                        business_sale::Call::BuildingAppearance(id) => {
                            (0x485520, vec![0x591318 + id as u32 * 916])
                        }
                        business_sale::Call::RefreshInterface => (0x41f410, vec![]),
                        business_sale::Call::BuildingPanel(id) => (0x407660, vec![id as u32]),
                        business_sale::Call::NativeUiCall {
                            address,
                            args,
                            argument_count,
                        } => (address, args[..usize::from(argument_count)].to_vec()),
                    });
                    Ok(())
                },
            )
            .unwrap();
        } else {
            actual_trace.push((0x480900, vec![0, 1225 + h[0], 0]));
        }
        let cash_trace: Vec<_> = expected_trace
            .iter()
            .filter(|call| matches!(call.0, 0x4ad760 | 0x4c6290 | 0x4c62d0))
            .collect();
        if !cash_trace.is_empty() {
            cash_cases += 1;
            assert_ne!(h[5], 0);
        }
        check_cash_presenter(case, &expected_trace);
        assert_eq!(
            actual_trace, expected_trace,
            "case{case} full callback order"
        );
        assert_eq!(u32::from(age), h[2]);
        assert_eq!(
            businesses[0].as_slice(),
            regions[0].1,
            "case{case} business"
        );
        assert_eq!(buildings[0].as_slice(), regions[1].1, "case{case} building");
        for (i, player) in players.iter().enumerate() {
            let mut actual = regions[5 + i].0.to_vec();
            player.write_runtime(&mut actual).unwrap();
            assert_eq!(actual, regions[5 + i].1, "case{case} player{i}");
        }
        for region in &regions[2..5] {
            assert_eq!(region.0, region.1, "case{case} immutable catalog");
        }
    }
    assert_eq!(
        cash_cases, 12,
        "current-owner callback boundary must be exercised"
    );
    assert_eq!(r.at, bytes.len());
}

fn check_cash_presenter(case: u32, trace: &[(u32, Vec<u32>)]) {
    let cash: Vec<_> = trace.iter().filter(|call|
        matches!(call.0, 0x4ad760 | 0x4c6290 | 0x4c62d0)).collect();
    let mut host = crate::live_host::Host::new([0.; 3]);
    let (sound, requests) = std::sync::mpsc::channel();
    host.gameplay_sound = Some(sound);
    // Native producer ordering enters the real saved-preview Host adapter.
    let calls = cash.iter().map(|(address, values)| {
        let mut args = [0; 6];
        args[..values.len()].copy_from_slice(values);
        openmt2001::category_ownership::Seam::NativeUiCall {
            address: *address, args, argument_count: values.len() as u8,
        }
    }).collect();
    host.apply_restored_presentation(calls).unwrap();
    let queued: Vec<_> = requests.try_iter().map(|request| request.operation).collect();
    let expected: Vec<_> = cash.iter().filter(|call| call.0 == 0x4ad760)
        .map(|call| crate::gameplay_sound::Operation::Ledger { sale: call.1[0] == 0x541cc8 })
        .collect();
    assert_eq!(queued, expected, "case{case} Host Ledger order");
    let first = cash.iter().find(|call| matches!(call.0, 0x4c6290 | 0x4c62d0));
    assert_eq!(host.cash_arrow_state().active, first.is_some());
    if let Some(pulse) = first {
        // Actual same-player takeovers include an opposite second pulse.
        // It cannot replace the first native sequence while the latch is set.
        assert_eq!(host.cash_arrow_state().frame, if pulse.0 == 0x4c6290 { 41 } else { 0 });
    }
}

#[test]
#[ignore = "requires original current-player takeover stream fixture"]
fn presents_native_takeover_cash_streams() {
    let bytes = std::fs::read("work/save/native-takeover-cash.bin").unwrap();
    let mut r = Reader { bytes: &bytes, at: 0 };
    assert_eq!(r.take(8), b"MTTAUI01");
    assert_eq!((r.word(), r.word()), (0x037f, 96));
    let mut both = 0;
    for case in 0..96 {
        let h: [u32; 9] = std::array::from_fn(|_| r.word());
        assert_eq!(h[6], 6);
        for _ in 0..h[6] { let n = r.word() as usize; r.take(n * 2); }
        let trace: Vec<_> = (0..h[7]).map(|_| {
            let address = r.word(); let count = r.word();
            (address, (0..count).map(|_| r.word()).collect::<Vec<_>>())
        }).collect();
        both += usize::from(trace.iter().filter(|call| call.0 == 0x4ad760).count() == 2);
        check_cash_presenter(case, &trace);
        r.take(48 + h[8] as usize * 52);
    }
    assert_eq!(both, 6);
    assert_eq!(r.at, bytes.len());
}

#[test]
#[ignore = "requires original sale through positive appearance stage"]
fn matches_native_sale_at_positive_appearance_boundary() {
    let bytes = std::fs::read("work/save/native-sale-positive-appearance.bin").unwrap();
    let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
    let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
    let mut r = Reader { bytes: &bytes, at: 0 };
    assert_eq!(r.take(8), b"MTSLAPP2");
    r.take(32); // The native probe verifies the pinned executable hash.
    assert_eq!((r.word(), r.word()), (0x037f, 15));
    let mut owned = 0;
    let mut active_rows = 0;
    for case in 0..15 {
        let slot = r.word() as usize;
        let _descriptor = r.word();
        let count = r.word();
        assert!(count > 0);
        for _ in 0..count {
            let entry = r.take(112);
            let before = &entry[8..60];
            let after = &entry[60..112];
            assert_eq!(&before[..40], &after[..40], "case{case} registry identity");
            assert_eq!(&before[44..], &after[44..], "case{case} registry geometry");
            if word(before, 0) == 1 {
                active_rows += 1;
                assert_ne!(&before[40..44], &after[40..44],
                    "case{case} active renderer handle should be replaced");
            } else {
                assert_eq!(&before[40..44], &after[40..44],
                    "case{case} inactive renderer handle should be retained");
            }
        }
        let prior_building = r.take(916);
        let building = r.take(916);
        let prior_business = r.take(544);
        let business = r.take(544);
        let prior_player = r.take(548);
        let player = r.take(548);
        owned += usize::from(prior_business[10] != 0);
        let id = word(prior_building, 0x1c) as usize;
        let mut buildings = vec![[0; 916]; 600];
        buildings[slot].copy_from_slice(prior_building);
        let mut businesses = vec![[0; 544]; 600];
        businesses[id].copy_from_slice(prior_business);
        let mut players = vec![openmt2001::economy::PlayerAccount::from_runtime(prior_player).unwrap()];
        // These tables are never consumed before the deliberately stopped
        // synchronous appearance boundary; do not invent stock initialization.
        let catalog = openmt2001::business_pool::BusinessCatalog::from_rows([[0; 116]; 45]);
        let mut age = 0;
        let mut reached = false;
        let result = business_sale::apply(&business_sale::Command { business: id as u32, player: 0 },
            &mut business_sale::World { businesses: &mut businesses, buildings: &mut buildings,
                players: &mut players, catalog: &catalog, products: &[[[0; 2]; 6]; 45],
                commodities: &[[0; 28]; 32], age: &mut age },
            &business_sale::Controls { current_player: None, accounting: false,
                ownership_tick: 500, timestamp: 1000, hour: 0 }, &image,
            |call, world| match call {
                business_sale::Call::BuildingAppearance(actual) if actual == slot => {
                    assert!(!reached); reached = true;
                    assert_eq!(world.buildings[slot].as_slice(), building, "case{case} native building at485520");
                    assert_eq!(world.businesses[id].as_slice(), business, "case{case} native business at485520");
                    let mut account = prior_player.to_vec();
                    world.players[0].write_runtime(&mut account)?;
                    assert_eq!(account, player, "case{case} native player at485520");
                    Err("checked appearance stop".into())
                }
                other => Err(format!("unexpected pre-appearance call {other:?}")),
            });
        assert_eq!(result.unwrap_err(), "checked appearance stop");
        assert!(reached);
    }
    assert_eq!(owned, 6);
    assert_eq!(active_rows, 45);
    assert_eq!(r.at, bytes.len());
}
