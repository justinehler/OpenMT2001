//! Original4896e0 periodic objective evaluation and489df0 result ordering.
use crate::{player_manager::Manager, scenario_objective::State, x87::Sum};

pub struct Controls {
    pub network_enabled: bool,
    pub network_override: bool,
    /// Original86d1fc, the remaining kind!=2 player count (manager+0c).
    pub mode: u32,
    pub year: u32,
    pub day: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Metric {
        address: u32,
        player: usize,
    },
    /// Original450490 command type9, arguments winner/reason; flag10 is1.
    Outcome {
        winner: i32,
        reason: u32,
    },
    /// Original48cab0(slot,-1,0x2d,0,0,0,0,-999), then48cc50(slot).
    RemovalCommand(usize),
    RemovePlayer(usize),
}
type Callback<'a> =
    dyn FnMut(Event, &mut State, &mut Manager, &mut Controls) -> Result<Sum, String> + 'a;
fn permitted(controls: &Controls) -> bool {
    !controls.network_enabled || controls.network_override
}
fn active(manager: &Manager, slot: usize) -> Result<bool, String> {
    Ok(*manager.players[slot]
        .bytes
        .get(0x10)
        .ok_or("truncated objective player")?
        != 0)
}

pub fn advance(
    state: &mut State,
    manager: &mut Manager,
    controls: &mut Controls,
    callback: &mut Callback<'_>,
) -> Result<(), String> {
    if !permitted(controls) {
        return Ok(());
    }
    if manager.players.len() > 6 {
        return Err("objective roster exceeds6 slots".into());
    }
    if state.flag != 0 && state.kind != 0 && controls.mode == 1 {
        for slot in 0..manager.players.len() {
            if active(manager, slot)? {
                callback(
                    Event::Outcome {
                        winner: slot as i32,
                        reason: 4,
                    },
                    state,
                    manager,
                    controls,
                )?;
                break;
            }
        }
        return Ok(());
    }
    let kind = state.kind;
    if kind == 0 {
        return Ok(());
    }
    let mut matches = 0;
    if matches!(kind, 3 | 5 | 6) {
        let mut slot = 0;
        while slot < manager.players.len() {
            if slot >= 6 {
                return Err("objective callback expanded roster beyond6 slots".into());
            }
            let mut value = if kind == 6 {
                let bytes = manager.players[slot]
                    .bytes
                    .get(0x1b4..0x1b8)
                    .ok_or("truncated objective quantity")?;
                Sum::from_i32(i32::from_le_bytes(bytes.try_into().unwrap()))
            } else {
                callback(
                    Event::Metric {
                        address: if kind == 3 { 0x47fca0 } else { 0x47fbb0 },
                        player: slot,
                    },
                    state,
                    manager,
                    controls,
                )?
            };
            let target = Sum::from_f32(state.target)?;
            if value.compare(target) != std::cmp::Ordering::Less {
                state.reached[slot] = 1;
                value.add_f32(-state.target)?;
                state.excess[slot] = value.ftol_low_word();
                matches += 1;
            }
            slot += 1;
        }
    } else if kind == 4 {
        let date = controls
            .year
            .wrapping_mul(10)
            .wrapping_add(1930)
            .wrapping_add(controls.day) as i32;
        if date >= crate::economy::ftol_low_word(f64::from(state.target)) {
            let mut best = -f32::MAX;
            let mut slot = 0;
            while slot < manager.players.len() {
                if active(manager, slot)? {
                    let value = callback(
                        Event::Metric {
                            address: 0x47fbb0,
                            player: slot,
                        },
                        state,
                        manager,
                        controls,
                    )?;
                    if value.compare(Sum::from_f32(best)?) == std::cmp::Ordering::Greater {
                        best = callback(
                            Event::Metric {
                                address: 0x47fbb0,
                                player: slot,
                            },
                            state,
                            manager,
                            controls,
                        )?
                        .ftol_low_word() as f32;
                    }
                }
                slot += 1;
            }
            let mut slot = 0;
            while slot < manager.players.len() {
                if slot >= 6 {
                    return Err("objective callback expanded roster beyond6 slots".into());
                }
                if active(manager, slot)? {
                    let value = callback(
                        Event::Metric {
                            address: 0x47fbb0,
                            player: slot,
                        },
                        state,
                        manager,
                        controls,
                    )?
                    .ftol_low_word();
                    if Sum::from_i32(value).compare(Sum::from_f32(best)?)
                        == std::cmp::Ordering::Equal
                    {
                        state.reached[slot] = 1;
                        matches += 1;
                    }
                }
                slot += 1;
            }
        }
    }
    resolve(state, manager, controls, matches, callback)
}

pub fn resolve(
    state: &mut State,
    manager: &mut Manager,
    controls: &mut Controls,
    matches: u32,
    callback: &mut Callback<'_>,
) -> Result<(), String> {
    if matches == 0 {
        return Ok(());
    }
    if manager.players.len() > 6 {
        return Err("objective result roster exceeds6 slots".into());
    }
    let mut winner = -1;
    let mut best = -9999;
    for slot in 0..manager.players.len() {
        if state.reached[slot] == 0 {
            continue;
        }
        if matches == 1 {
            if active(manager, slot)? {
                winner = slot as i32;
            }
        } else {
            let value = state.excess[slot];
            if value == best {
                winner = -1;
            }
            if value > best {
                best = value;
                winner = slot as i32;
            }
        }
    }
    if permitted(controls) {
        callback(
            Event::Outcome { winner, reason: 2 },
            state,
            manager,
            controls,
        )?;
    }
    if matches != 1 && winner == -1 {
        return Ok(());
    }
    let mut slot = 0;
    while slot < manager.players.len() {
        if slot as i32 != winner && (matches != 1 || active(manager, slot)?) && permitted(controls)
        {
            callback(Event::RemovalCommand(slot), state, manager, controls)?;
            callback(Event::RemovePlayer(slot), state, manager, controls)?;
        }
        slot += 1;
    }
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
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires original4896e0/489df0 observations"]
    fn matches_native_objective_ticks() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_OBJECTIVE_TICK")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(7), b"MTOBJ02");
        assert_eq!(r.word(), 2);
        let cases = r.word();
        for _ in 0..cases {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let _kind = r.word();
            let count = r.word() as usize;
            let result_count = r.word();
            let _target = r.word();
            let year = r.word();
            let day = r.word();
            let mut controls = Controls {
                year,
                day,
                network_enabled: r.word() != 0,
                network_override: r.word() != 0,
                mode: r.word(),
            };
            assert_eq!(r.word(), 1, "{name}");
            assert_eq!(r.word(), 0, "{name}");
            let mut manager = Manager::initialize([0; 40]);
            for slot in 0..count {
                manager.players.push(crate::player_manager::Player {
                    reference: 0x10010000 + slot as u32 * 0x400,
                    bytes: r.blob().to_vec(),
                    auxiliaries: vec![],
                });
            }
            let expected: Vec<_> = (0..count).map(|_| r.blob()).collect();
            let mut state = State::from_bytes(&r.blob()[..60]).unwrap();
            let expected_state = r.blob();
            for _ in 0..2 {
                let n = r.word();
                r.take(n as usize * 12);
            }
            let globals: [u32; 5] = std::array::from_fn(|_| r.word());
            let n = r.word();
            let mut expected_trace = Vec::new();
            for _ in 0..n {
                let kind = r.word();
                let address = r.word();
                let _object = r.word();
                let n = r.word();
                let args: Vec<_> = (0..n).map(|_| r.word()).collect();
                expected_trace.push(match kind {
                    1 => {
                        let winner = r.word() as i32;
                        let reason = r.word();
                        let descriptor = r.blob();
                        assert_eq!(&descriptor[4..8], &9u32.to_le_bytes());
                        Event::Outcome { winner, reason }
                    }
                    3 => {
                        assert_eq!(args[1..], [u32::MAX, 0x2d, 0, 0, 0, 0, (-999i32) as u32]);
                        Event::RemovalCommand(args[0] as usize)
                    }
                    4 => Event::RemovePlayer(args[0] as usize),
                    6 => Event::Metric {
                        address,
                        player: args[0] as usize,
                    },
                    _ => panic!("unexpected native objective call {address:x} in {name}"),
                });
            }
            let mut trace = Vec::new();
            let mut callback = |event, _: &mut State, manager: &mut Manager, _: &mut Controls| {
                trace.push(event);
                if let Event::Metric { address, player } = event {
                    let bytes = &manager.players[player].bytes;
                    if address == 0x47fca0 {
                        return crate::player_metrics::current_income(
                            bytes[4..444].try_into().unwrap(),
                        );
                    }
                    let offset = 4;
                    Sum::from_f32(f32::from_le_bytes(
                        bytes[offset..offset + 4].try_into().unwrap(),
                    ))
                    .map_err(str::to_owned)
                } else {
                    Ok(Sum::from_i32(0))
                }
            };
            let result = if entry == 0 {
                advance(&mut state, &mut manager, &mut controls, &mut callback)
            } else {
                resolve(
                    &mut state,
                    &mut manager,
                    &mut controls,
                    result_count,
                    &mut callback,
                )
            };
            if name.contains("nan_input") || name.contains("inf_input") {
                assert!(
                    result.is_err(),
                    "nonfinite objective metric is explicitly unsupported: {name}"
                );
                continue;
            }
            result.unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(trace, expected_trace, "{name}");
            assert_eq!(state.to_bytes(), expected_state[..60], "{name}");
            for (player, expected) in manager.players.iter().zip(expected) {
                assert_eq!(player.bytes, expected, "{name}");
            }
            assert_eq!(
                [
                    manager.players.len() as u32,
                    controls.mode,
                    u32::from(controls.network_enabled),
                    u32::from(controls.network_override)
                ],
                [globals[0], globals[2], globals[3], globals[4]],
                "{name}"
            );
        }
        assert!(r.0.is_empty());
    }
}
