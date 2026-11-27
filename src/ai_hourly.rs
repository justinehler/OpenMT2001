//! Original00470b80 hourly AI pass and its00466bf0 per-player body.
//!
//! Eligibility, timers and event auxiliary transitions use retained records.
//! Event target selection and retirement remain synchronous callbacks.

use crate::player_manager::{Player, put, word};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub player: usize,
}

pub struct Callbacks<'a> {
    pub update: &'a mut dyn FnMut(Call, &mut Player) -> Result<(), String>,
    pub delay: &'a mut dyn FnMut(Call) -> Result<u32, String>,
}

/// Original004769f0 ignores its ECX caller and reads global86ce60. Modes one
/// and two return AL=1; every other mode returns AL=0.
pub fn delay_signal(mode: u32) -> u8 {
    u8::from(matches!(mode, 1 | 2))
}

/// Execute original00470b80 over the live player table.
pub fn advance(
    players: &mut [Player],
    current_hour: u32,
    callbacks: &mut Callbacks<'_>,
) -> Result<(), String> {
    for (index, player) in players.iter_mut().enumerate() {
        // 00470b80 only reads +0x10 and +0x154 while selecting AI records.
        // Human roster records can end after this prefix and are skipped
        // without requiring the larger AI controller allocation.
        if player.bytes.len() < 0x158 {
            continue;
        }
        if player.bytes[0x10] == 0 || word(&player.bytes, 0x154) != 1 {
            continue;
        }
        if player.bytes.len() < 0x1e50 {
            return Err(format!("truncated eligible hourly AI player {index}"));
        }
        advance_player(index, player, current_hour, callbacks)?;
    }
    Ok(())
}

fn advance_player(
    index: usize,
    player: &mut Player,
    current_hour: u32,
    callbacks: &mut Callbacks<'_>,
) -> Result<(), String> {
    // 00466bf0 decrements the 6x0x34 dword matrix at +0x143c, stepping by 8
    // bytes.  The test is signed (`jle`), so negative values are preserved.
    for slot in 0..(6 * 0x34) {
        let offset = 0x143c + slot * 8;
        let value = word(&player.bytes, offset);
        if (value as i32) > 0 {
            put(&mut player.bytes, offset, value.wrapping_sub(1));
        }
    }

    (callbacks.update)(
        Call {
            address: 0x0046f020,
            player: index,
        },
        player,
    )?;

    if word(&player.bytes, 0xb54) == 1 {
        let value = word(&player.bytes, 0xb8c);
        put(&mut player.bytes, 0xb8c, value.wrapping_add(1));
    }

    let value = word(&player.bytes, 0x13e4);
    if (value as i32) > 0 {
        put(&mut player.bytes, 0x13e4, value.wrapping_sub(1));
    }

    let value = word(&player.bytes, 0x1e14);
    if (value as i32) > 0 {
        put(&mut player.bytes, 0x1e14, value.wrapping_sub(1));
    } else {
        let returned = (callbacks.delay)(Call {
            address: 0x004769f0,
            player: index,
        })?;
        let delay = if returned as u8 == 0 { 2 } else { 23 };
        put(&mut player.bytes, 0x1e14, delay);
        player.bytes[0x221] |= 4;
    }

    if word(&player.bytes, 0x1e14) == 1 {
        player.bytes[0x221] |= 8;
    }
    player.bytes[0x1e34] = u8::from(current_hour == 6);
    Ok(())
}

/// Downstream branches of46f020 not yet bound to the shared event-card logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateCall {
    Evaluate,
    Retire { account: u32 },
}

/// Original46f020 with native4727b0,41bc60 and41bd30(2) reads/stores.
/// The auxiliary is captured before any downstream calls, matching EDI.
pub fn update(
    player: &mut Player,
    cash_statuses: &[u8; 6],
    image: &crate::pe::PeImage<'_>,
    mut callback: impl FnMut(UpdateCall, &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    if player.bytes.len() < 0x13e4 {
        return Err("AI hourly update record truncated".into());
    }
    let reference = word(&player.bytes, 0x21c);
    let age = word(&player.bytes, 0x13e0);
    if (age as i32) > 0 {
        put(&mut player.bytes, 0x13e0, age.wrapping_add(1));
    }
    if (word(&player.bytes, 0x13e0) as i32) > 24 {
        player.bytes[0x13dd] = 1;
    }
    let account = player
        .auxiliaries
        .iter()
        .position(|a| a.reference == reference)
        .ok_or("AI hourly account reference unresolved")?;
    match word(&player.auxiliaries[account].bytes, 0x18) {
        2 => callback(UpdateCall::Evaluate, player),
        1 => {
            let status = *cash_statuses
                .get(player.bytes[0x1bc] as usize)
                .ok_or("AI hourly cash-status slot outside table")?;
            let cash = f32::from_bits(word(&player.bytes, 4));
            let threshold = image.f32_at(0x525bac)?;
            if status != 0 && (cash.is_nan() || threshold.is_nan() || cash < threshold) {
                return Ok(());
            }
            let age = word(&player.bytes, 0x13e0).wrapping_add(1);
            put(&mut player.bytes, 0x13e0, age);
            let entry = word(&player.auxiliaries[account].bytes, 4);
            let address = entry
                .checked_mul(20)
                .and_then(|v| v.checked_add(0x534208))
                .ok_or("AI hourly event table address overflow")?;
            let kind = u32::from_le_bytes(image.at(address, 4)?.try_into().unwrap());
            if kind != 2 {
                callback(UpdateCall::Retire { account: reference }, player)?;
                put(&mut player.bytes, 0x13e0, 0);
                player.bytes[0x13dd] = 0;
            } else {
                let owner = word(&player.auxiliaries[account].bytes, 0x14);
                if owner != player.reference {
                    return Err("AI hourly event auxiliary has foreign owner".into());
                }
                put(&mut player.bytes, 0x1b8, entry);
                put(&mut player.auxiliaries[account].bytes, 0x18, 2);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_manager::put;
    use std::cell::RefCell;

    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, length: usize) -> &'a [u8] {
            let (head, tail) = self.0.split_at(length);
            self.0 = tail;
            head
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> Vec<u8> {
            let length = self.word() as usize;
            self.take(length).to_vec()
        }
    }

    #[test]
    #[ignore = "requires original00470b80 hourly AI observations"]
    fn matches_native_hourly_ai_pass() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_HOURLY")).unwrap();
        let mut reader = Reader(&bytes);
        assert_eq!(reader.take(8), b"MTAIH470");
        assert_eq!(reader.word(), 1);
        let case_count = reader.word();
        for case in 0..case_count {
            let name = String::from_utf8(reader.blob()).unwrap();
            let hour = reader.word();
            let player_count = reader.word() as usize;
            let mut players = Vec::with_capacity(player_count);
            let mut configurations = Vec::with_capacity(player_count);
            let mut expected = Vec::with_capacity(player_count);
            for index in 0..player_count {
                let active = reader.word();
                let kind = reader.word();
                let b54 = reader.word();
                let counter = reader.word();
                let delay_return = reader.word();
                let mutation_b54 = reader.word();
                let mutation_1e14 = reader.word();
                let flags221 = reader.word();
                let record_length = reader.word() as usize;
                let before = reader.blob();
                let after = reader.blob();
                assert_eq!(before.len(), 0x1e50);
                assert_eq!(after.len(), 0x1e50);
                configurations.push((
                    active,
                    kind,
                    b54,
                    counter,
                    delay_return,
                    mutation_b54,
                    mutation_1e14,
                    flags221,
                    record_length,
                ));
                players.push(Player {
                    reference: 0x10010000 + (index as u32) * 0x3000,
                    bytes: before[..record_length.min(before.len())].to_vec(),
                    auxiliaries: vec![],
                });
                expected.push(after);
            }
            let references: Vec<u32> = players.iter().map(|player| player.reference).collect();
            let expected_hour = reader.word();
            let return_reached = reader.word() != 0;
            let fault = reader.word() != 0;
            let call_count = reader.word() as usize;
            let expected_calls: Vec<(u32, usize, u32)> = (0..call_count)
                .map(|_| {
                    let call = (reader.word(), reader.word() as usize, reader.word());
                    let _reserved = reader.word();
                    call
                })
                .collect();
            let actual_calls = RefCell::new(Vec::new());
            let mut update = |call: Call, player: &mut Player| {
                let config = configurations[call.player];
                if config.5 != u32::MAX {
                    put(&mut player.bytes, 0xb54, config.5);
                }
                if config.6 != u32::MAX {
                    put(&mut player.bytes, 0x1e14, config.6);
                }
                actual_calls
                    .borrow_mut()
                    .push((call.address, call.player, player.reference));
                Ok(())
            };
            let mut delay = |call: Call| {
                actual_calls.borrow_mut().push((
                    call.address,
                    call.player,
                    references[call.player],
                ));
                Ok(configurations[call.player].4)
            };
            let mut callbacks = Callbacks {
                update: &mut update,
                delay: &mut delay,
            };
            advance(&mut players, hour, &mut callbacks).unwrap();
            assert!(
                return_reached && !fault,
                "native case {case} {name} did not return"
            );
            assert_eq!(expected_hour, hour, "hour case {case} {name}");
            assert_eq!(
                actual_calls.into_inner(),
                expected_calls,
                "calls case {case} {name}"
            );
            for (index, (player, expected)) in players.iter().zip(expected).enumerate() {
                assert_eq!(
                    player.bytes,
                    expected[..player.bytes.len()].to_vec(),
                    "record case {case} {name} player {index}"
                );
            }
            for (index, config) in configurations.iter().enumerate() {
                assert_eq!(player_byte(&players[index], 0x10), config.0 as u8);
                assert_eq!(word(&players[index].bytes, 0x154), config.1);
            }
        }
        assert!(reader.0.is_empty());
    }

    #[test]
    #[ignore = "requires original46f020 auxiliary state observations"]
    fn matches_native_hourly_update() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_HOURLY_UPDATE")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTAIU602");
        assert_eq!(r.word(), 2);
        for _ in 0..r.word() {
            let name = String::from_utf8(r.blob()).unwrap();
            let statuses: [u8; 6] = r.take(6).try_into().unwrap();
            let mutations: Vec<_> = (0..r.word())
                .map(|_| {
                    let b = r.take(4);
                    let target = b[0];
                    let width = b[1];
                    let offset = u16::from_le_bytes(b[2..4].try_into().unwrap()) as usize;
                    (target, width, offset, r.word())
                })
                .collect();
            let plen = r.word() as usize;
            let alen = r.word() as usize;
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.take(plen).to_vec(),
                auxiliaries: vec![crate::player_manager::Auxiliary {
                    reference: 0x10014000,
                    bytes: r.take(alen).try_into().unwrap(),
                }],
            };
            let plen = r.word() as usize;
            let alen = r.word() as usize;
            let expected_player = r.take(plen);
            let expected_aux = r.take(alen);
            assert_eq!(r.word(), 1, "return {name}");
            assert_eq!(r.word(), 0, "fault {name}");
            let calls = r.word();
            r.word();
            let expected_trace: Vec<_> = (0..calls).map(|_| (r.word(), r.word())).collect();
            let mut trace = vec![];
            update(&mut player, &statuses, &image, |call, p| {
                let target = match call {
                    UpdateCall::Evaluate => {
                        trace.push((0x46f130, p.reference));
                        0
                    }
                    UpdateCall::Retire { account } => {
                        trace.push((0x41bc90, account));
                        1
                    }
                };
                for &(m_target, width, offset, value) in &mutations {
                    if m_target != target {
                        continue;
                    }
                    let b = if target == 0 {
                        &mut p.bytes[..]
                    } else {
                        &mut p.auxiliaries[0].bytes[..]
                    };
                    b[offset..offset + width as usize]
                        .copy_from_slice(&value.to_le_bytes()[..width as usize]);
                }
                Ok(())
            })
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(trace, expected_trace, "trace {name}");
            assert_eq!(player.bytes, expected_player, "player {name}");
            assert_eq!(
                player.auxiliaries[0].bytes, expected_aux,
                "auxiliary {name}"
            );
        }
        assert!(r.0.is_empty());
    }

    fn player_byte(player: &Player, offset: usize) -> u8 {
        player.bytes[offset]
    }
}
