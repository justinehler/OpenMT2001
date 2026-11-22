//! Original47c5f0 offer admission and469850 retained counterparty history.
use crate::player_manager::{Player, put, word};

pub fn eligible(player: &Player, category: u32, amount: u32) -> Result<bool, String> {
    let at = 0x1150_i64 + i64::from(category as i32) * 12;
    let at = usize::try_from(at).map_err(|_| "offer eligibility history underflow")?;
    let row = player
        .bytes
        .get(at..at + 8)
        .ok_or("offer eligibility history leaves player")?;
    if row[0] == 0 {
        return Ok(false);
    }
    let threshold =
        crate::economy::ftol_low_word(f64::from(f32::from_bits(word(row, 4)))).wrapping_mul(5);
    Ok(amount as i32 > threshold)
}
pub fn remember(
    player: &mut Player,
    category: u32,
    amount: u32,
    recipient: i32,
    profiles: &[[u8; 0x650]],
) -> Result<(), String> {
    if category > 52 || !(-1..=6).contains(&recipient) {
        return Err("original469850 invalid history diagnostic".into());
    }
    let at = 0x1438_i64 + (i64::from(category) + i64::from(recipient) * 52) * 8;
    let at = usize::try_from(at).map_err(|_| "offer history underflow")?;
    if at + 8 > player.bytes.len() {
        return Err("offer history leaves player allocation".into());
    }
    // No arithmetic occurs between the native binary32 load and store.
    put(&mut player.bytes, at, amount);
    let age = super::auction::inbox::history_age(player, profiles)?;
    put(&mut player.bytes, at + 4, age);
    Ok(())
}
///477be0's accepted offer creates this original52-byte local auction request.
pub fn request(arguments: [u32; 8], mut prior: [u8; 52]) -> [u8; 52] {
    for (at, value) in [(0, 0x525cb8), (4, 0x1d), (12, 0)] {
        put(&mut prior, at, value);
    }
    prior[16] = 1;
    for (i, value) in arguments.into_iter().enumerate() {
        put(&mut prior, 20 + i * 4, value);
    }
    prior
}

#[test]
#[ignore = "requires original offer negotiation captures"]
fn matches_native_negotiation() {
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_OFFER_NEGOTIATION").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = crate::pe::PeImage::parse(&exe).unwrap();
    assert_eq!(&bytes[..8], b"MTAONEG1");
    assert_eq!(word(&bytes, 8), 0x37f);
    let mut cursor = 16;
    for case in 0..word(&bytes, 12) {
        let mut take = |n| {
            let b = &bytes[cursor..cursor + n];
            cursor += n;
            b
        };
        let c = take(20);
        let prior = take(52).try_into().unwrap();
        let packet: [u8; 48] = take(48).try_into().unwrap();
        let mut player = Player {
            reference: 0x10010000,
            bytes: take(0x1e50).to_vec(),
            auxiliaries: vec![crate::player_manager::Auxiliary {
                reference: 0x10030000,
                bytes: packet,
            }],
        };
        let expected = take(0x1e50);
        let result = take(8);
        let mut traces = vec![];
        for _ in 0..word(result, 4) {
            let config = take(8);
            traces.push((word(config, 0), take(word(config, 4) as usize).to_vec()));
        }
        let mut profiles = vec![[0; 0x650]; 10];
        profiles[word(&player.bytes, 0x150) as usize]
            [0x5bb + word(&player.bytes, 0x13f4) as usize * 36] = word(c, 16) as u8;
        match word(c, 0) {
            0x47c5f0 => assert_eq!(
                u32::from(eligible(&player, word(c, 4), word(c, 8)).unwrap()),
                word(result, 0),
                "{case} eligible"
            ),
            0x469850 => {
                let amount = word(&player.bytes, 0x1154 + word(c, 4) as usize * 12);
                remember(
                    &mut player,
                    word(c, 4),
                    amount,
                    word(c, 12) as i32,
                    &profiles,
                )
                .unwrap();
            }
            0x477be0 => {
                let mut actual = vec![];
                super::predecision(
                    &mut player,
                    1,
                    0x10030000,
                    None,
                    &[0; 6],
                    &image,
                    |call, player| {
                        match call {
                            super::Call::Eligible { target, amount } => {
                                return Ok(super::Reply {
                                    value: u32::from(eligible(player, target, amount)?),
                                    ..Default::default()
                                });
                            }
                            super::Call::Submit(args) => {
                                actual.push((0x450490, request(args, prior).to_vec()))
                            }
                            super::Call::Reply { packet, kind } => actual.push((
                                0x48cb70,
                                [packet, kind, 0x5f5e0ff, 0x5f5e0ff, 0x5f5e0ff, 0x5f5e0ff]
                                    .into_iter()
                                    .flat_map(u32::to_le_bytes)
                                    .collect(),
                            )),
                            other => panic!("{case} unexpected{other:?}"),
                        }
                        Ok(Default::default())
                    },
                )
                .unwrap();
                assert_eq!(actual, traces, "{case} outgoing calls");
            }
            _ => unreachable!(),
        }
        let diff: Vec<_> = player
            .bytes
            .iter()
            .zip(expected)
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some((i, *a, *b)))
            .collect();
        assert!(diff.is_empty(), "{case} state{diff:x?}");
    }
    assert_eq!(cursor, bytes.len());
}
