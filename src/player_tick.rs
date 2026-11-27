//! Original0048cea0 periodic player dispatch, with live callback effects.
use crate::player_manager::{Manager, put, word};

pub struct Controls {
    pub network_enabled: bool,
    pub network_override: bool,
    pub current_player: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Controller {
        player: u32,
        target: u32,
    },
    Ui {
        address: u32,
        arguments: [u32; 3],
        count: usize,
    },
}

pub fn advance(
    manager: &mut Manager,
    controls: &mut Controls,
    image: &crate::pe::PeImage<'_>,
    mut callback: impl FnMut(Event, &mut Manager, &mut Controls) -> Result<(), String>,
) -> Result<(), String> {
    let threshold = image.f32_at(0x525040)?;
    let mut index = 0;
    while index < manager.players.len() {
        let player = &manager.players[index];
        if player.bytes.len() < 0x1c4 {
            return Err("truncated periodic player".into());
        }
        if player.bytes[0x10] != 0 {
            if word(&player.bytes, 0x154) != 1
                || !controls.network_enabled
                || controls.network_override
            {
                let address = word(&player.bytes, 0)
                    .checked_add(8)
                    .ok_or("player vtable overflow")?;
                let target = u32::from_le_bytes(image.at(address, 4)?.try_into().unwrap());
                callback(
                    Event::Controller {
                        player: player.reference,
                        target,
                    },
                    manager,
                    controls,
                )?;
            }
            // Reload the table slot after the controller, then keep its object
            // identity through UI callbacks even if current-player changes.
            let player = manager
                .players
                .get(index)
                .ok_or("player callback removed current table slot")?;
            if player.reference == controls.current_player {
                let reference = player.reference;
                let cash = f32::from_bits(word(&player.bytes, 4));
                let previous = f32::from_bits(word(&player.bytes, 0x1c0));
                let below = |v: f32| v < threshold || v.is_nan() || threshold.is_nan();
                if below(cash) && !below(previous) {
                    for (address, arguments, count) in [
                        (0x43be20, [0, 0, 0], 0),
                        (0x429af0, [0x12e, 0, 0], 1),
                        (0x429820, [0, 0x6cb, 0], 3),
                        (0x429820, [1, 0x6cc, 0], 3),
                        (0x429a60, [1, 0, 0], 1),
                        (0x4298b0, [0, 0x59b, 0], 2),
                    ] {
                        callback(
                            Event::Ui {
                                address,
                                arguments,
                                count,
                            },
                            manager,
                            controls,
                        )?;
                    }
                }
                let player = manager
                    .players
                    .iter_mut()
                    .find(|p| p.reference == reference)
                    .ok_or("cash notification released the captured player")?;
                let cash = word(&player.bytes, 4);
                put(&mut player.bytes, 0x1c0, cash);
            }
        }
        index += 1;
    }
    Ok(())
}

/// Human0048c140 updates every second controller visit. Mode0 tail-calls
/// virtual+4 (0048bc70), which must be supplied by the live command handler.
/// Unexpected modes retain the reset store before the diagnostic boundary.
pub fn advance_human(
    player: &mut crate::player_manager::Player,
    mut commands: impl FnMut(&mut crate::player_manager::Player) -> Result<(), String>,
) -> Result<(), String> {
    if player.bytes.len() < 0x224 {
        return Err("truncated human controller".into());
    }
    if word(&player.bytes, 0) != 0x525e04 {
        return Err("unexpected human controller vtable".into());
    }
    let countdown = word(&player.bytes, 8).wrapping_sub(1);
    put(&mut player.bytes, 8, countdown);
    if countdown as i32 <= 0 {
        put(&mut player.bytes, 8, 2);
        match word(&player.bytes, 0x220) {
            // Original48bc70 returns immediately for an empty command queue.
            // Nonempty command dispatch remains synchronous and explicit.
            0 if word(&player.bytes, 0x214) != 0 => commands(player)?,
            0 => {}
            1 => {}
            _ => return Err("human controller requires original4e6960 diagnostic".into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (result, rest) = self.0.split_at(n);
            self.0 = rest;
            result
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires original player/controller dispatch observations"]
    fn matches_native_player_ticks() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PLAYER_TICK")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTPPLAY3");
        assert_eq!(r.word(), 3);
        let count = r.word();
        let mut tested = [0; 2];
        for _ in 0..count {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let config: [u32; 6] = std::array::from_fn(|_| r.word());
            let count = r.word() as usize;
            let current = r.word();
            let mutation_count = r.word();
            let mutations: Vec<[u32; 4]> = (0..mutation_count)
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut manager = Manager::initialize([0; 40]);
            let mut expected = Vec::new();
            for slot in 0..count {
                manager.players.push(crate::player_manager::Player {
                    reference: 0x10010000 + slot as u32 * 0x400,
                    bytes: r.blob().to_vec(),
                    auxiliaries: Vec::new(),
                });
                expected.push(r.blob());
            }
            r.blob();
            r.blob(); // objective evaluator is a separate subsystem
            let globals: [u32; 5] = std::array::from_fn(|_| r.word());
            let calls = r.word();
            let expected_trace: Vec<_> = (0..calls)
                .map(|_| {
                    let address = r.word();
                    let object = r.word();
                    let n = r.word();
                    let args: Vec<_> = (0..n).map(|_| r.word()).collect();
                    (address, object, args)
                })
                .collect();
            if entry == 0 {
                tested[0] += 1;
                let mut controls = Controls {
                    network_enabled: config[0] != 0,
                    network_override: config[1] != 0,
                    current_player: current,
                };
                let mut trace = Vec::new();
                advance(&mut manager, &mut controls, &image, |event, manager, _| {
                    match event {
                        Event::Controller { player, target } => {
                            trace.push((target, player, Vec::new()));
                            for &[slot, offset, width, value] in &mutations {
                                if offset == 0x86d1f8 {
                                    manager.players.truncate(value as usize);
                                } else {
                                    let bytes = &mut manager.players[slot as usize].bytes;
                                    bytes[offset as usize..offset as usize + width as usize]
                                        .copy_from_slice(&value.to_le_bytes()[..width as usize]);
                                }
                            }
                        }
                        Event::Ui {
                            address,
                            arguments,
                            count,
                        } => trace.push((address, 0x10013000, arguments[..count].to_vec())),
                    }
                    Ok(())
                })
                .unwrap();
                assert_eq!(trace, expected_trace, "{name}");
                assert_eq!(
                    [
                        manager.players.len() as u32,
                        controls.current_player,
                        u32::from(controls.network_enabled),
                        u32::from(controls.network_override)
                    ],
                    globals[..4],
                    "{name}"
                );
            } else if entry == 2 {
                tested[1] += 1;
                let result = advance_human(&mut manager.players[0], |_| {
                    Err("unexpected nonempty native command queue".into())
                });
                assert_eq!(
                    result.is_err(),
                    expected_trace.iter().any(|(a, _, _)| *a == 0x4e6960),
                    "{name}"
                );
            } else {
                continue;
            }
            for (player, expected) in manager.players.iter().zip(expected) {
                assert_eq!(player.bytes, expected, "{name}");
            }
        }
        assert!(tested[0] >= 10 && tested[1] >= 4);
        assert!(r.0.is_empty());
    }
}
