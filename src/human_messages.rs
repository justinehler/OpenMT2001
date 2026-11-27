//! Original48bc70 human inbox presentation and41bb20 card receipt.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    player_messages::HumanCall,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Ui {
        address: u32,
        arguments: [u32; 4],
        count: usize,
    },
    /// Original stores to the live modal callback and optional context word.
    Handler {
        callback: u32,
        context: Option<u32>,
    },
    Diagnostic,
}
#[derive(Clone, Copy)]
pub struct Actor {
    pub reference: u32,
    pub character: u32,
}
pub struct World<'a, 'data> {
    pub image: &'a PeImage<'data>,
    pub actors: &'a [Actor],
    pub categories: &'a [[u8; 380]],
    pub buildings: &'a [[u8; 0x394]],
    pub businesses: &'a [[u8; 0x220]],
    pub catalog: &'a [[u8; 116]],
}
fn packet(player: &Player) -> Result<&[u8; 48], String> {
    let reference = word(&player.bytes, 0x20c);
    player
        .auxiliaries
        .iter()
        .find(|a| a.reference == reference)
        .map(|a| &a.bytes)
        .ok_or("human current packet unresolved".into())
}
fn sender<'a>(player: &Player, world: &World<'a, '_>) -> Result<Actor, String> {
    world
        .actors
        .get(packet(player)?[0x14] as i8 as usize)
        .copied()
        .ok_or("human message sender outside roster".into())
}
fn title(player: &Player, world: &World<'_, '_>, city: bool) -> Result<u32, String> {
    if city && packet(player)?[0x14] == 0xff {
        return Ok(0x12e);
    }
    Ok(word(
        world
            .image
            .at(0x541afc + sender(player, world)?.character * 40, 4)?,
        0,
    ))
}
fn category_name(category: u32, world: &World<'_, '_>) -> Result<u32, String> {
    Ok(word(
        world
            .image
            .at(0x5385a0u32.wrapping_add(category.wrapping_mul(88)), 4)?,
        0,
    ))
}
fn packet_category_name(player: &Player, world: &World<'_, '_>) -> Result<u32, String> {
    let category = world
        .categories
        .get(word(packet(player)?, 0x18) as usize)
        .ok_or("human property message category outside pool")?;
    category_name(word(category, 0x8c), world)
}

/// UI callbacks remain synchronous; current packet fields are reread after each
/// original callback. The renderer owns modal callback/context storage; Handler
/// reports the exact writes without retaining a second copy of the modal queue.
pub fn consume(
    player: &mut Player,
    world: &World<'_, '_>,
    mut callback: impl FnMut(Call, &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    crate::player_messages::consume_human(player, |call, player| match call {
        HumanCall::Ui {
            address,
            arguments,
            count,
        } => callback(
            Call::Ui {
                address,
                arguments: [arguments[0], arguments[1], arguments[2], 0],
                count,
            },
            player,
        ),
        HumanCall::Diagnostic => callback(Call::Diagnostic, player),
        HumanCall::Packet { kind: 0x33, .. } => {
            let card = word(packet(player)?, 0x18);
            crate::event_cards::lifecycle::receive(player, word(&player.bytes, 0x21c), card)
        }
        HumanCall::Packet { kind, .. } => {
            let mut ui = |address: u32, args: &[u32], player: &mut Player| {
                let mut arguments = [0; 4];
                arguments[..args.len()].copy_from_slice(args);
                callback(
                    Call::Ui {
                        address,
                        arguments,
                        count: args.len(),
                    },
                    player,
                )
            };
            match kind {
                0x24 | 0x26 => {
                    ui(0x43be20, &[], player)?;
                    ui(0x429af0, &[title(player, world, true)?], player)?;
                    ui(
                        0x429820,
                        &[0, if kind == 0x24 { 0x6ac } else { 0x6ae }, 0],
                        player,
                    )?;
                    ui(0x441940, &[], player)?;
                    ui(
                        0x441080,
                        &[
                            if kind == 0x24 { 0x6ad } else { 0x6af },
                            packet_category_name(player, world)?,
                        ],
                        player,
                    )?;
                    ui(0x429a60, &[1], player)?;
                    ui(0x4298b0, &[0, 0x59b], player)
                }
                0x23 => {
                    ui(0x43be20, &[], player)?;
                    ui(0x429af0, &[title(player, world, false)?], player)?;
                    ui(0x429820, &[0, 0x6b0, 0], player)?;
                    ui(0x429a60, &[2], player)?;
                    ui(0x4298b0, &[0, 0x6a8], player)?;
                    ui(0x4298b0, &[1, 0x6a9], player)?;
                    ui(0x441940, &[], player)?;
                    let category = packet_category_name(player, world)?;
                    let price = word(packet(player)?, 0x1c);
                    ui(
                        0x440d40,
                        &[
                            0x6b1,
                            sender(player, world)?.reference + 0x158,
                            price,
                            category,
                        ],
                        player,
                    )?;
                    callback(
                        Call::Handler {
                            callback: 0x48b350,
                            context: Some(word(packet(player)?, 4)),
                        },
                        player,
                    )?;
                    callback(
                        Call::Ui {
                            address: 0x429ac0,
                            arguments: [0; 4],
                            count: 1,
                        },
                        player,
                    )?;
                    put(&mut player.bytes, 0x220, 1);
                    Ok(())
                }
                0x2e => {
                    let building = world
                        .buildings
                        .get(word(packet(player)?, 0x18) as usize)
                        .ok_or("human takeover building outside pool")?;
                    let index = word(building, 0x1c);
                    if index == u32::MAX {
                        return Ok(());
                    }
                    let business = world
                        .businesses
                        .get(index as usize)
                        .ok_or("human takeover business outside pool")?;
                    ui(0x43be20, &[], player)?;
                    ui(0x429af0, &[title(player, world, false)?], player)?;
                    ui(0x429820, &[0, 0x6b2, 0], player)?;
                    ui(0x429a60, &[1], player)?;
                    ui(0x4298b0, &[0, 0x59b], player)?;
                    ui(0x441940, &[], player)?;
                    let building = i16::from_le_bytes(business[8..10].try_into().unwrap());
                    let category = if building == -1 {
                        -1
                    } else {
                        let b = world
                            .buildings
                            .get(building as usize)
                            .ok_or("human takeover linked building outside pool")?;
                        let offset = word(b, 0x40)
                            .checked_sub(0x617628)
                            .ok_or("human takeover category pointer before pool")?;
                        if !offset.is_multiple_of(380) {
                            return Err("human takeover category pointer unaligned".into());
                        }
                        let row = world
                            .categories
                            .get((offset / 380) as usize)
                            .ok_or("human takeover category pointer outside pool")?;
                        i16::from_le_bytes(row[..2].try_into().unwrap())
                    };
                    let category = category_name(category as u32, world)?;
                    let kind = word(business, 4);
                    let business_name = if kind == u32::MAX {
                        0x35e
                    } else {
                        word(
                            world
                                .catalog
                                .get(kind as usize)
                                .ok_or("human takeover kind outside catalog")?,
                            4,
                        )
                    };
                    ui(
                        0x440d40,
                        &[
                            0x6b3,
                            sender(player, world)?.reference + 0x158,
                            business_name,
                            category,
                        ],
                        player,
                    )?;
                    callback(
                        Call::Handler {
                            callback: 0x48b480,
                            context: None,
                        },
                        player,
                    )?;
                    put(&mut player.bytes, 0x220, 1);
                    Ok(())
                }
                _ => Err(format!("human packet kind{kind:x} has no native branch")),
            }
        }
    })
}

#[test]
#[ignore = "requires whole original48bc70 human observations"]
fn matches_native_messages() {
    let data = std::fs::read(crate::fixtures::path("NATIVE_HUMAN_MESSAGES")).unwrap();
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    assert_eq!(&data[..8], b"MTHUMAN1");
    assert_eq!(word(&data, 8), 0x37f);
    let mut at = 16;
    for _ in 0..word(&data, 12) {
        let seed = word(&data, at);
        at += 4;
        let mut b = vec![];
        for _ in 0..15 {
            let n = word(&data, at) as usize;
            at += 4;
            b.push(&data[at..at + n]);
            at += n;
        }
        let count = word(&data, at);
        at += 4;
        let mut expected = vec![];
        for _ in 0..count {
            let address = word(&data, at);
            let n = word(&data, at + 4) as usize;
            at += 8;
            let mut row = vec![address, n as u32];
            for _ in 0..n {
                row.push(word(&data, at));
                at += 4;
            }
            expected.push(row);
        }
        let mut player = Player {
            reference: 0x10010000,
            bytes: b[0].to_vec(),
            auxiliaries: vec![
                crate::player_manager::Auxiliary {
                    reference: 0x10014000,
                    bytes: b[3].try_into().unwrap(),
                },
                crate::player_manager::Auxiliary {
                    reference: 0x10015000,
                    bytes: b[4].try_into().unwrap(),
                },
                crate::player_manager::Auxiliary {
                    reference: 0x10016000,
                    bytes: b[5].try_into().unwrap(),
                },
            ],
        };
        let actors: Vec<_> = (0..3)
            .map(|i| Actor {
                reference: 0x10010000 + i as u32 * 0x1000,
                character: word(b[i], 0x150),
            })
            .collect();
        let categories: Vec<[u8; 380]> = b[6]
            .chunks_exact(380)
            .map(|r| r.try_into().unwrap())
            .collect();
        let buildings: Vec<[u8; 916]> = b[7]
            .chunks_exact(916)
            .map(|r| r.try_into().unwrap())
            .collect();
        let businesses: Vec<[u8; 544]> = b[8]
            .chunks_exact(544)
            .map(|r| r.try_into().unwrap())
            .collect();
        let catalog: Vec<[u8; 116]> = b[9]
            .chunks_exact(116)
            .map(|r| r.try_into().unwrap())
            .collect();
        let world = World {
            image: &image,
            actors: &actors,
            categories: &categories,
            buildings: &buildings,
            businesses: &businesses,
            catalog: &catalog,
        };
        let mut dialog = [word(b[10], 0), word(b[10], 12)];
        let mut trace = vec![];
        consume(&mut player, &world, |call, p| {
            match call {
                Call::Ui {
                    address,
                    arguments,
                    count,
                } => {
                    let mut row = vec![address, count as u32];
                    row.extend_from_slice(&arguments[..count]);
                    trace.push(row);
                    if address == 0x43be20 && seed.is_multiple_of(5) {
                        let packet = &mut p
                            .auxiliaries
                            .iter_mut()
                            .find(|a| a.reference == 0x10014000)
                            .unwrap()
                            .bytes;
                        packet[0x14] = ((seed + 2) % 3) as u8;
                        put(packet, 0x18, (seed + 1) % 4);
                    }
                }
                Call::Diagnostic => trace.push(vec![0x4e6960, 3, 1, 0, 0x541d34]),
                Call::Handler { callback, context } => {
                    dialog[0] = callback;
                    if let Some(context) = context {
                        dialog[1] = context;
                    }
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(trace, expected, "case{seed} UItrace");
        assert_eq!(player.bytes, b[11], "case{seed} player");
        assert_eq!(
            &player
                .auxiliaries
                .iter()
                .find(|a| a.reference == 0x10014000)
                .unwrap()
                .bytes,
            b[12],
            "case{seed} packet"
        );
        assert_eq!(
            &player
                .auxiliaries
                .iter()
                .find(|a| a.reference == 0x10016000)
                .unwrap()
                .bytes,
            b[13],
            "case{seed} event"
        );
        assert_eq!(
            dialog,
            [word(b[14], 0), word(b[14], 12)],
            "case{seed} dialog"
        );
    }
    assert_eq!(at, data.len());
}
