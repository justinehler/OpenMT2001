//! Original46a200, signed bid increments and per-player RNG.

use crate::player_manager::{put, word};
pub fn budget(player: &mut crate::player_manager::Player, auction: &[u8; 128]) -> u32 {
    let player = &mut player.bytes;
    let high = (word(auction, 0x68) as i32).max(word(auction, 0x6c) as i32);
    if player[0xcd0] != 0 {
        let count = word(player, 0xcc4) as i32;
        let amount = high.wrapping_add(50);
        if count > 0 && amount <= word(player, 0xcc8) as i32 {
            put(player, 0xcc4, count.wrapping_sub(1) as u32);
            return amount as u32;
        }
        return 0;
    }
    let step = if word(player, 0xb64) == 1 {
        2
    } else {
        let next = word(player, 0x224)
            .wrapping_mul(word(player, 0x228))
            .wrapping_add(word(player, 0x22c));
        put(player, 0x224, next);
        let choice = ((next >> 16) & 0x7fff) % 101;
        if choice > 90 {
            2
        } else {
            u32::from(choice > 60)
        }
    };
    let limit = f64::from(f32::from_bits(word(player, 0x1dfc)));
    let mut amount = high.wrapping_add(match step {
        2 => 500,
        1 => 100,
        _ => 50,
    });
    if step == 2 && f64::from(amount) > limit {
        amount = high.wrapping_add(100);
    }
    if step != 0 && f64::from(amount) > limit {
        amount = high.wrapping_add(50);
    }
    if f64::from(amount) > limit {
        0
    } else {
        amount as u32
    }
}
#[test]
#[ignore = "requires original auction budget capture"]
fn matches_native_budget() {
    let data = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_BUDGET")).unwrap();
    assert_eq!(&data[..8], b"MTAOBD01");
    assert_eq!(word(&data, 8), 0x037f);
    let count = word(&data, 12);
    let mut data = &data[16..];
    for case in 0..count {
        let mut player = data[..0x1e50].to_vec();
        let auction = &data[0x1e50..0x1ed0];
        let expected = &data[0x1ed0..0x3d20];
        let returned = word(data, 0x3d20);
        let mut runtime = crate::player_manager::Player {
            reference: 0x10010000,
            bytes: player,
            auxiliaries: vec![],
        };
        let actual = budget(&mut runtime, auction.try_into().unwrap());
        player = runtime.bytes;
        assert_eq!(actual, returned, "case{case}");
        assert_eq!(player, expected, "case{case}");
        data = &data[0x3d24..];
    }
    assert!(data.is_empty());
}
