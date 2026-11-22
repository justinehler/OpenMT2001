//! Original447e20 hourly lease countdown over authoritative category records.
use crate::{
    category::{Call, Controls},
    category_ownership::World,
    pe::PeImage,
    player_manager::{put, word},
};

/// Callbacks include48c450 release and464e30 auction cleanup. They execute
/// synchronously against the same pools, before the next original reload.
pub fn advance(
    world: &mut World<'_>,
    controls: &mut Controls,
    image: &PeImage<'_>,
    mut call: impl FnMut(Call, &mut World<'_>, &mut Controls) -> Result<(), String>,
) -> Result<(), String> {
    for category in 0..52 {
        if world.categories[category][5] == 0 || word(&world.categories[category], 0x7c) == 0 {
            continue;
        }
        for at in [0x7c, 0x80] {
            let v = word(&world.categories[category], at).wrapping_sub(1);
            put(&mut world.categories[category], at, v);
        }
        let owned = |world: &World<'_>, controls: &Controls| {
            i32::from(world.categories[category][5] as i8)
                == i32::from(controls.current_owner_index) + 1
        };
        if word(&world.categories[category], 0x7c) == 24 && owned(world, controls) {
            notify(category, 0x6c0, world, controls, image, &mut call)?;
        }
        if word(&world.categories[category], 0x7c) != 0 {
            continue;
        }
        if owned(world, controls) {
            notify(category, 0x6c1, world, controls, image, &mut call)?;
        }
        call(
            Call {
                address: 0x48c450,
                arguments: [category as u32, 0, 0],
                argument_count: 2,
            },
            world,
            controls,
        )?;
        if controls.selection_active && controls.selected_category == Some(category as u32) {
            call(
                Call {
                    address: 0x464e30,
                    arguments: [0; 3],
                    argument_count: 0,
                },
                world,
                controls,
            )?;
        }
    }
    Ok(())
}
fn notify(
    category: usize,
    event: u32,
    world: &mut World<'_>,
    controls: &mut Controls,
    image: &PeImage<'_>,
    call: &mut impl FnMut(Call, &mut World<'_>, &mut Controls) -> Result<(), String>,
) -> Result<(), String> {
    for (address, arguments, argument_count) in [
        (0x43be20, [0; 3], 0),
        (0x429af0, [0x12e, 0, 0], 1),
        (0x429820, [0, 0x6bf, 0], 3),
        (0x441940, [0; 3], 0),
    ] {
        call(
            Call {
                address,
                arguments,
                argument_count,
            },
            world,
            controls,
        )?;
    }
    let index = word(&world.categories[category], 0x8c);
    let name = word(
        image.at(0x5385a0u32.wrapping_add(index.wrapping_mul(88)), 4)?,
        0,
    );
    for (address, arguments, argument_count) in [
        (0x441080, [event, name, 0], 2),
        (0x429a60, [1, 0, 0], 1),
        (0x4298b0, [0, 0x59b, 0], 2),
    ] {
        call(
            Call {
                address,
                arguments,
                argument_count,
            },
            world,
            controls,
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "requires original447e20/48c450 lease observations"]
fn matches_native_live_leases() {
    use crate::{category_ownership::Seam, player_manager::Player};
    struct R<'a>(&'a [u8]);
    impl<'a> R<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            word(self.take(4), 0)
        }
    }
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_CATEGORY_LEASE").unwrap()).unwrap();
    let mut r = R(&data);
    assert_eq!(r.take(8), b"MTLEASE1");
    assert_eq!(r.word(), 0x37f);
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    for case in 0..r.word() {
        let h: [u32; 9] = std::array::from_fn(|_| r.word());
        let mut before = vec![];
        let mut after = vec![];
        for _ in 0..4 {
            let n = r.word() as usize;
            before.push(r.take(n));
            after.push(r.take(n));
        }
        let expected: Vec<Vec<u32>> = (0..h[8])
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut categories = Box::new(std::array::from_fn(|i| {
            before[0][i * 380..(i + 1) * 380].try_into().unwrap()
        }));
        let mut buildings: Vec<[u8; 916]> = before[1]
            .chunks_exact(916)
            .map(|b| b.try_into().unwrap())
            .collect();
        let mut businesses: Vec<[u8; 544]> = before[2]
            .chunks_exact(544)
            .map(|b| b.try_into().unwrap())
            .collect();
        let mut players = vec![Player {
            reference: 0x10020000,
            bytes: before[3].to_vec(),
            auxiliaries: vec![],
        }];
        let chains =
            crate::building_pool::category_chains(&categories, &buildings, 0x591318).unwrap();
        let mut world = World {
            categories: &mut categories,
            buildings: &mut buildings,
            businesses: &mut businesses,
            players: &mut players,
            chains: &chains,
            normal_tick: h[1],
            category_tick: 0,
            refresh_counter: h[3] as u8,
            accounting_enabled: h[2] != 0,
            current_player: None,
            notification_threshold: image.f32_at(0x525040).unwrap(),
        };
        let mut controls = Controls {
            current_owner_index: h[4] as u8,
            selected_category: Some(h[0]),
            selection_active: h[5] != 0,
        };
        let mut trace = vec![];
        advance(&mut world, &mut controls, &image, |call, world, _| {
            if call.address == 0x48c450 {
                world.release(call.arguments[0] as usize, 0., &mut |world, seam| {
                    trace.push(match seam {
                        Seam::BuildingAppearance { building } => {
                            vec![0x485520, 0x591318 + building as u32 * 916]
                        }
                        Seam::AiOwnershipLists { player } => {
                            vec![0x467a30, world.players[player].reference]
                        }
                        Seam::NativeUiCall {
                            address: 0x41f410, ..
                        } => vec![0x41f410],
                        other => panic!("lease unexpected seam{other:?}"),
                    });
                    Ok(())
                })?;
            } else {
                let mut row = vec![call.address];
                row.extend(&call.arguments[..call.argument_count as usize]);
                trace.push(row);
                if h[6] != 0 && call.address == 0x441940 {
                    put(&mut world.categories[h[0] as usize], 0x7c, 0);
                    put(&mut world.categories[h[0] as usize], 0x8c, (h[0] + 1) % 52);
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            world.refresh_counter,
            u8::try_from(h[7]).unwrap(),
            "case{case} age"
        );
        assert_eq!(trace, expected, "case{case} trace");
        assert_eq!(categories.as_flattened(), after[0], "case{case} categories");
        assert_eq!(buildings.as_flattened(), after[1], "case{case} buildings");
        assert_eq!(businesses.as_flattened(), after[2], "case{case} businesses");
        assert_eq!(players[0].bytes, after[3], "case{case} player");
    }
    assert!(r.0.is_empty());
}
