//! Original48b030/48b130/48b140 consumer poll on the authoritative live pools.
use crate::{
    ai_strategy_queries::visit_chain,
    economy::ResidentAccounts,
    player_manager::{Player, put, word},
};
fn increment(player: &mut Player, at: usize) {
    let v = u16::from_le_bytes(player.bytes[at..at + 2].try_into().unwrap()).wrapping_add(1);
    player.bytes[at..at + 2].copy_from_slice(&v.to_le_bytes());
}
///48b030 accumulates into existing counters; it does not clear a previous poll.
///Invalid category IDs return before changing either flag. Resident activity,
///business ownership and account activity do not filter the original chain.
pub fn start(
    player: &mut Player,
    category: i32,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    businesses: &[[u8; 544]],
    residents: &ResidentAccounts,
    commodity_active: &[bool; 32],
) -> Result<(), String> {
    let Some(row) = usize::try_from(category)
        .ok()
        .and_then(|i| categories.get(i))
    else {
        return Ok(());
    };
    player.bytes[0x12] = 1;
    player.bytes[0x11] = 1;
    put(&mut player.bytes, 0x1c4, category as u32);
    visit_chain(row, buildings, |building| {
        let id = word(building, 0x1c) as i32;
        if id == -1 {
            return Ok(());
        }
        let business = businesses
            .get(id as usize)
            .ok_or("consumer poll business leaves pool")?;
        if word(business, 4) != 42 {
            return Ok(());
        }
        let mut next = word(business, 0x20c) as i32;
        let mut visited = 0;
        while next != -1 {
            if visited >= residents.nodes.len() {
                return Err("consumer poll resident chain cycles".into());
            }
            visited += 1;
            let node = residents
                .nodes
                .get(next as usize)
                .ok_or("consumer poll resident leaves pool")?
                .as_bytes();
            let account = i16::from_le_bytes(node[0x68..0x6a].try_into().unwrap());
            let bytes = residents
                .accounts
                .get(account as usize)
                .ok_or("consumer poll account unresolved (original would read invalid address)")?
                .as_bytes();
            let mask = word(bytes, 0x84);
            if player.bytes[0x12] != 0 {
                increment(player, 0x208);
                increment(player, 0x20a);
                for (id, &active) in commodity_active.iter().enumerate() {
                    if active && mask & (1u32 << id) != 0 {
                        increment(player, 0x1c8 + id * 2);
                    }
                }
            }
            next = word(node, 0xec) as i32;
        }
        Ok(())
    })?;
    player.bytes[0x12] = 0;
    Ok(())
}
///48b130 stops collection while retaining the accumulated result.
pub fn stop(player: &mut Player) {
    player.bytes[0x12] = 0;
}
///48b140 resets only the original flags/category/count fields.
pub fn clear(player: &mut Player) {
    player.bytes[0x11] = 0;
    player.bytes[0x12] = 0;
    put(&mut player.bytes, 0x1c4, 0);
    player.bytes[0x1c8..0x20c].fill(0);
}
#[test]
#[ignore = "requires original live consumer poll capture"]
fn matches_native_live_poll() {
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_LIVE_CONSUMER_POLL").unwrap()).unwrap();
    let mut data = &bytes[..];
    fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (a, b) = data.split_at(n);
        *data = b;
        a
    }
    assert_eq!(take(&mut data, 8), b"MTLPOL01");
    assert_eq!(word(take(&mut data, 4), 0), 0x37f);
    let count = word(take(&mut data, 4), 0);
    for case in 0..count {
        let category = word(take(&mut data, 4), 0) as i32;
        let mut player = Player {
            reference: 0x10010000,
            bytes: take(&mut data, 0x1e50).to_vec(),
            auxiliaries: vec![],
        };
        let categories: Box<[[u8; 380]; 52]> = take(&mut data, 52 * 380)
            .chunks_exact(380)
            .map(|r| r.try_into().unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap();
        let buildings: Vec<[u8; 916]> = take(&mut data, 12 * 916)
            .chunks_exact(916)
            .map(|r| r.try_into().unwrap())
            .collect();
        let businesses: Vec<[u8; 544]> = take(&mut data, 12 * 544)
            .chunks_exact(544)
            .map(|r| r.try_into().unwrap())
            .collect();
        let nodes = take(&mut data, 36 * 330)
            .chunks_exact(330)
            .map(|r| crate::resident::Resident::from_pool_bytes(r.try_into().unwrap(), 0))
            .collect();
        let accounts = take(&mut data, 36 * 144)
            .chunks_exact(144)
            .map(|r| crate::resident::SpendingAccount::from_bytes(r.try_into().unwrap()))
            .collect();
        let residents = ResidentAccounts {
            active_count: 0,
            nodes,
            accounts,
        };
        let raw = take(&mut data, 32);
        let active = std::array::from_fn(|i| raw[i] != 0);
        for operation in 0..4 {
            match operation {
                0 | 2 => start(
                    &mut player,
                    category,
                    &categories,
                    &buildings,
                    &businesses,
                    &residents,
                    &active,
                )
                .unwrap(),
                1 => stop(&mut player),
                _ => clear(&mut player),
            }
            assert_eq!(
                &player.bytes[..],
                take(&mut data, 0x1e50),
                "case {case}, operation {operation}"
            );
        }
    }
    assert!(data.is_empty());
}
