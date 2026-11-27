//! Original48b350 offer answer and48b480 takeover-notice dismissal.
use crate::{
    player_manager::{Player, put, word},
    player_messages::Packet,
};
pub struct World<'a> {
    pub categories: &'a mut [[u8; 380]; 52],
    pub player_references: &'a [u32],
}
pub struct Inputs {
    pub button: u32,
    pub message_kind: u32,
    pub command_prior: [u8; 52],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Refuse {
        packet_reference: u32,
        packet: Packet,
    },
    Auction {
        descriptor: [u8; 52],
    },
}
pub fn dismiss_notice(player: &mut Player) -> Result<(), String> {
    if player.bytes.len() < 0x224 {
        return Err("human reply player truncated".into());
    }
    put(&mut player.bytes, 0x220, 0);
    Ok(())
}
/// The owner's identity is rechecked when the answer arrives. All paths clear
/// the busy DWORD, after the synchronous reply or auction submission completes.
pub fn offer_response(
    player: &mut Player,
    world: &mut World<'_>,
    input: &Inputs,
    mut callback: impl FnMut(Call, &mut Player, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    if player.bytes.len() < 0x224 {
        return Err("human reply player truncated".into());
    }
    let reference = word(&player.bytes, 0x20c);
    let packet = crate::ai_messages::offers::packet(player, reference)?;
    let category = world
        .categories
        .get(word(&packet, 0x18) as usize)
        .ok_or("human offer category outside pool")?;
    let owner = category[5] as i8;
    let owner = if owner == 0 {
        0
    } else {
        *world
            .player_references
            .get((i32::from(owner) - 1) as usize)
            .ok_or("human offer owner outside manager")?
    };
    if owner == player.reference && input.message_kind == 0x23 {
        match input.button {
            0 => {
                let category = word(&packet, 0x18);
                let amount = word(&packet, 0x1c);
                let descriptor = crate::ai_messages::offers::negotiation::request(
                    [
                        category,
                        i32::from(packet[0x14] as i8) as u32,
                        amount,
                        category,
                        amount,
                        word(&packet, 0x20),
                        word(&packet, 0x24),
                        word(&packet, 0x28),
                    ],
                    input.command_prior,
                );
                callback(Call::Auction { descriptor }, player, world)?;
            }
            1 => callback(
                Call::Refuse {
                    packet_reference: reference,
                    packet,
                },
                player,
                world,
            )?,
            _ => (),
        }
    }
    dismiss_notice(player)
}

#[test]
#[ignore = "requires original48b350/48b480 whole response observations"]
fn matches_native_human_replies() {
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
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    let d = std::fs::read(std::env::var("OPENMT2001_NATIVE_HUMAN_REPLY").unwrap()).unwrap();
    let mut r = R(&d);
    assert_eq!(r.take(8), b"MTHREPL1");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 5] = std::array::from_fn(|_| r.word());
        let prior = r.blob().try_into().unwrap();
        let reply_prior = r.blob().try_into().unwrap();
        let source: Packet = r.blob().try_into().unwrap();
        let b: Vec<_> = (0..8).map(|_| r.blob()).collect();
        let expected: Vec<_> = (0..h[3])
            .map(|_| {
                let address = r.word();
                let busy = r.word();
                (address, busy, r.blob().to_vec())
            })
            .collect();
        let nodes: Vec<_> = (0..h[4])
            .map(|_| {
                let address = r.word();
                (address, r.blob())
            })
            .collect();
        let mut players: Vec<_> = (0..3)
            .map(|i| Player {
                reference: 0x10030000 + i as u32 * 0x400,
                bytes: b[i].to_vec(),
                auxiliaries: if i == 0 {
                    vec![crate::player_manager::Auxiliary {
                        reference: 0x10050000,
                        bytes: source,
                    }]
                } else {
                    vec![]
                },
            })
            .collect();
        let references: Vec<_> = players.iter().map(|p| p.reference).collect();
        let mut categories = Box::new([[0; 380]; 52]);
        let category_id = word(&source, 0x18) as usize;
        categories[category_id] = b[3].try_into().unwrap();
        let mut world = World {
            categories: &mut categories,
            player_references: &references,
        };
        let mut arena = crate::player_messages::Arena { next: 0x10060000 };
        let mut actual = vec![];
        let (before, tail) = players.split_at_mut(0);
        let (player, after) = tail.split_first_mut().unwrap();
        if h[0] == 0x48b480 {
            dismiss_notice(player).unwrap();
        } else {
            offer_response(
                player,
                &mut world,
                &Inputs {
                    button: h[1],
                    message_kind: h[2],
                    command_prior: prior,
                },
                |call, p, w| {
                    let busy = word(&p.bytes, 0x220);
                    match call {
                        Call::Auction { descriptor } => {
                            actual.push((0x450490, busy, descriptor.to_vec()))
                        }
                        Call::Refuse {
                            packet_reference,
                            packet,
                        } => {
                            actual.push((
                                0x48cb70,
                                busy,
                                [
                                    packet_reference,
                                    0x26,
                                    0x5f5e0ff,
                                    0x5f5e0ff,
                                    0x5f5e0ff,
                                    0x5f5e0ff,
                                ]
                                .into_iter()
                                .flat_map(u32::to_le_bytes)
                                .collect(),
                            ));
                            crate::player_messages::reply_with_players(
                                &packet,
                                0x26,
                                [0x5f5e0ff; 4],
                                &mut crate::player_messages::SplitPlayers {
                                    before,
                                    current: p,
                                    after,
                                },
                                &mut arena,
                                crate::player_messages::Routing {
                                    current_player: 0x10030000,
                                    network: [false, false],
                                    prior: reply_prior,
                                },
                                |route, _, peers| {
                                    match route {
                                        crate::player_messages::Route::PropertyRefresh {
                                            category,
                                        } => {
                                            actual.push((
                                                0x405260,
                                                word(&peers.current.bytes, 0x220),
                                                [category, 1]
                                                    .into_iter()
                                                    .flat_map(u32::to_le_bytes)
                                                    .collect(),
                                            ));
                                            assert!(crate::category_ownership::property_view(
                                                &mut w.categories[category as usize],
                                                1,
                                                1
                                            ));
                                        }
                                        other => {
                                            return Err(format!(
                                                "unexpected human refusal external route{other:?}"
                                            ));
                                        }
                                    }
                                    Ok(())
                                },
                            )?;
                        }
                    }
                    Ok(())
                },
            )
            .unwrap_or_else(|e| panic!("case{case}: {e}"));
        }
        assert_eq!(
            actual, expected,
            "case{case} exact response order/busy latch"
        );
        assert_eq!(
            world.categories[category_id].as_slice(),
            b[7],
            "case{case} retained category"
        );
        for (i, p) in players.iter().enumerate() {
            assert_eq!(p.bytes, b[4 + i], "case{case} player{i}");
        }
        assert_eq!(
            players[0].auxiliaries[0].bytes, source,
            "original retained source unchanged"
        );
        let actual_nodes: Vec<_> = players
            .iter()
            .flat_map(|p| {
                p.auxiliaries
                    .iter()
                    .filter(|a| a.reference >= 0x10060000)
                    .map(|a| (a.reference, a.bytes.as_slice()))
            })
            .collect();
        assert_eq!(actual_nodes, nodes, "case{case} actual refusal inbox");
    }
    assert!(r.0.is_empty());
}
