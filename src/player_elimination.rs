//! Original48ca40/48af20 elimination, retaining allocated player slots.
use crate::player_manager::Manager;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    BusinessOwner { business: usize },
    BuildingOwner { building: usize },
    BuildingRefresh { building: usize },
    Auction,
    PlayerUi { object: usize },
    NetworkFinalize,
    FinishLocal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Auction {
    pub reference: u32,
    pub category: i32,
    pub player: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controls {
    pub network_enabled: bool,
    pub current_player: u32,
    pub auction: Option<Auction>,
    /// Original82296e and82296b refresh bytes.
    pub refresh: [u8; 2],
}
pub struct Pools<'a> {
    pub categories: &'a mut [[u8; 0x17c]],
    pub businesses: &'a mut [[u8; 0x220]],
    pub buildings: &'a mut [[u8; 0x394]],
}
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
///450470 disables the optional5Hz450410 command/network task; it leaves the
/// task allocated and preserves its retained identity.
pub fn finish_local(task:Option<usize>,timers:&mut [crate::timers::Task])->Result<(),String> {
    if let Some(index)=task {timers.get_mut(index).ok_or("local command timer outside manager")?[0]=0;}
    Ok(())
}
pub fn advance(
    slot: usize,
    manager: &mut Manager,
    mut pools: Pools<'_>,
    controls: &mut Controls,
    mut callback: impl FnMut(Call, &mut Manager, &mut Pools<'_>, &mut Controls) -> Result<(), String>,
) -> Result<(), String> {
    let player = manager
        .players
        .get(slot)
        .ok_or("elimination player missing")?;
    if player.bytes.len() < 0x1bd {
        return Err("elimination player truncated".into());
    }
    if player.bytes[0x10] == 0 {
        return Ok(());
    }
    if pools.categories.len() < 52 || pools.businesses.len() < 600 || pools.buildings.len() < 600 {
        return Err("elimination pools truncated".into());
    }
    manager.players[slot].bytes[0x10] = 0;
    manager.players[slot].bytes[0x154..0x158].copy_from_slice(&2u32.to_le_bytes());
    for category in 0..52 {
        let owner = i32::from(manager.players[slot].bytes[0x1bc]) + 1;
        if i32::from(pools.categories[category][5] as i8) == owner {
            pools.categories[category][5] = 0;
            pools.categories[category][0x78..0x7c].copy_from_slice(&2u32.to_le_bytes());
            manager.players[slot].bytes[0x178 + category] = 0;
        }
    }
    for business in 0..600 {
        let owner = i32::from(manager.players[slot].bytes[0x1bc]) + 1;
        let row = &pools.businesses[business];
        if row[0x21c] == 0 || i32::from(row[0xa] as i8) != owner {
            continue;
        }
        let building = i16::from_le_bytes(row[8..10].try_into().unwrap()) as usize;
        let row = pools
            .buildings
            .get(building)
            .ok_or("elimination building leaves pool")?;
        let linked = word(row, 0x1c) as usize;
        if linked >= pools.businesses.len() {
            return Err("elimination linked business leaves pool".into());
        }
        callback(
            Call::BusinessOwner { business: linked },
            manager,
            &mut pools,
            controls,
        )?;
        callback(
            Call::BuildingOwner { building },
            manager,
            &mut pools,
            controls,
        )?;
        callback(
            Call::BuildingRefresh { building },
            manager,
            &mut pools,
            controls,
        )?;
    }
    if let Some(auction) = controls.auction {
        let owner = u32::from(manager.players[slot].bytes[0x1bc]);
        let affected = if auction.player == owner {
            true
        } else {
            let category = pools
                .categories
                .get(auction.category as usize)
                .ok_or("elimination auction category outside pool")?;
            (i32::from(category[5] as i8) - 1) as u32 == owner
        };
        if affected {
            callback(Call::Auction, manager, &mut pools, controls)?;
        }
    }
    for value in &mut controls.refresh {
        *value = value.wrapping_add(100);
    }
    if manager.players[slot].reference == controls.current_player {
        callback(Call::PlayerUi { object: 0 }, manager, &mut pools, controls)?;
        callback(Call::PlayerUi { object: 1 }, manager, &mut pools, controls)?;
        if controls.network_enabled {
            callback(Call::NetworkFinalize, manager, &mut pools, controls)?;
        }
        callback(Call::FinishLocal, manager, &mut pools, controls)?;
    }
    let count = word(&manager.bytes, 12).wrapping_sub(1);
    manager.bytes[12..16].copy_from_slice(&count.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        fn controls(&mut self) -> Controls {
            let v: [u32; 7] = std::array::from_fn(|_| self.word());
            Controls {
                network_enabled: v[0] != 0,
                current_player: v[1],
                auction: (v[2] != 0).then_some(Auction {
                    reference: v[2],
                    category: v[3] as i32,
                    player: v[4],
                }),
                refresh: [v[5] as u8, v[6] as u8],
            }
        }
    }
    #[test]
    #[ignore = "requires original48ca40/48af20 pool observations"]
    fn matches_native_elimination() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PLAYER_ELIMINATION").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTELIM01");
        let count = r.word();
        const SIZE: usize = 40 + 6 * 544 + 52 * 380 + 600 * 544 + 600 * 916;
        for _ in 0..count {
            let n = r.word() as usize;
            let name = std::str::from_utf8(r.take(n)).unwrap();
            let slot = r.word() as usize;
            let mutation = r.word();
            let mut controls = r.controls();
            let before = r.take(SIZE);
            let expected_controls = r.controls();
            let after = r.take(SIZE);
            let n = r.word();
            let expected_calls: Vec<Vec<u32>> = (0..n)
                .map(|_| {
                    let n = r.word();
                    (0..n).map(|_| r.word()).collect()
                })
                .collect();
            let mut b = Reader(before);
            let mut manager = Manager {
                bytes: b.take(40).try_into().unwrap(),
                players: (0..6)
                    .map(|i| crate::player_manager::Player {
                        reference: 0x10010000 + i * 0x300,
                        bytes: b.take(544).to_vec(),
                        auxiliaries: vec![],
                    })
                    .collect(),
            };
            let mut categories: Vec<[u8; 380]> =
                (0..52).map(|_| b.take(380).try_into().unwrap()).collect();
            let mut businesses: Vec<[u8; 544]> =
                (0..600).map(|_| b.take(544).try_into().unwrap()).collect();
            let mut buildings: Vec<[u8; 916]> =
                (0..600).map(|_| b.take(916).try_into().unwrap()).collect();
            let mut calls = vec![];
            advance(
                slot,
                &mut manager,
                Pools {
                    categories: &mut categories,
                    businesses: &mut businesses,
                    buildings: &mut buildings,
                },
                &mut controls,
                |call, manager, pools, controls| {
                    calls.push(match call {
                        Call::BusinessOwner { business } => {
                            pools.businesses[business][10] = 0;
                            if mutation == 1 {
                                manager.players[slot].bytes[0x1bc] = 5;
                            }
                            vec![0x462b00, 0x7a41a8 + business as u32 * 544, 0]
                        }
                        Call::BuildingOwner { building } => {
                            pools.buildings[building][0x2d] = 0;
                            pools.buildings[building][0x30..0x34]
                                .copy_from_slice(&777u32.to_le_bytes());
                            vec![0x4454c0, 0x591318 + building as u32 * 916, 0]
                        }
                        Call::BuildingRefresh { building } => {
                            vec![0x4450a0, 0x591318 + building as u32 * 916]
                        }
                        Call::Auction => {
                            controls.auction = None;
                            vec![0x464e30, 0x10014000]
                        }
                        Call::PlayerUi { object } => {
                            if mutation == 2 {
                                controls.network_enabled = true;
                            }
                            vec![0x43bed0, 0x10016000 + object as u32 * 0x1000]
                        }
                        Call::NetworkFinalize => vec![0x4e1db0, 0x93b278, 0],
                        Call::FinishLocal => vec![0x450470, 0],
                    });
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(calls, expected_calls, "{name} callbacks");
            assert_eq!(controls, expected_controls, "{name} controls");
            let mut actual = manager.bytes.to_vec();
            for player in &manager.players {
                actual.extend_from_slice(&player.bytes);
            }
            actual.extend_from_slice(categories.as_flattened());
            actual.extend_from_slice(businesses.as_flattened());
            actual.extend_from_slice(buildings.as_flattened());
            if let Some(at) = actual.iter().zip(after).position(|(a, b)| a != b) {
                panic!(
                    "{name} pool mismatch offset {at:x}: {} != {}",
                    actual[at], after[at]
                );
            }
        }
        assert!(r.0.is_empty());
    }
}
