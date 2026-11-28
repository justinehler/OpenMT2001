//! Original004899e0 player market-share pass.
//!
//! The first two stages are stateful and shared: 0048a580 clamps each active
//! player's `+1ac + +1b0` signed counters at zero, the total is accumulated
//! with x87 PC64 arithmetic, and normalized shares are FTOL-rounded before
//! being converted back to f32 for the downstream profile/effect callbacks.
//! Presentation and objective callbacks remain explicit events.

use crate::x87::Sum;

pub const PLAYER_SLOTS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub manager_kind: i32,
    pub manager_value: f32,
    pub day: u32,
    pub player_count: usize,
    pub network_enabled: bool,
    pub network_override: bool,
    pub current_slot: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResultState {
    /// The integer returned by each original 0050e928 conversion before the
    /// later `*0.01` f32 store. Inactive slots retain zero.
    pub shares: [i32; PLAYER_SLOTS],
    /// The f32 values passed to the original per-player profile/effect call.
    pub normalized: [f32; PLAYER_SLOTS],
    pub total: i32,
    /// Native 00489adf..00489b0d winner scans. Ties retain the last slot
    /// encountered, exactly as the original EBP/stack-index stores do.
    pub max_ties: usize,
    pub max_index: usize,
    pub min_ties: usize,
    pub min_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// Original48a150(message, detail), emitted by the manager state branch.
    Objective { message: u32, detail: u32 },
    /// Original43be20 manager refresh before profile/effect updates.
    Refresh,
    /// Original42c4b0(slot, normalized_f32, player_auxiliary_reference).
    PlayerShare {
        slot: usize,
        normalized: f32,
        auxiliary_reference: u32,
    },
    /// Original42c470 selector used by the kind2 one-winner branch.
    ProfileSelect { index: u32 },
    /// Original42c430 selector used by the kind1 one-winner branch.
    Kind1ProfileSelect { index: u32 },
    /// Original48cab0 command packet. The native call receives eight dwords.
    Command { args: [u32; 8] },
    /// Original450490 producer used when the network gate is clear.
    CommandProducer { slot: u32 },
    /// Original48cc50 player removal notification.
    RemovePlayer { slot: u32 },
    /// The late controller helper receives a literal one in this branch.
    LateController { argument: u32 },
}

/// Optional state changes returned by a host callback. Native callees can
/// update the live gate bytes, manager value, or player auxiliary before the
/// caller re-reads them; the comparison harness uses this to exercise those
/// order-sensitive paths.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CallbackEffect {
    pub return_value: i32,
    pub manager_value: Option<f32>,
    pub network_enabled: Option<bool>,
    pub network_override: Option<bool>,
    pub current_slot: Option<usize>,
    pub player_auxiliary: Option<(usize, u32)>,
    pub player_byte: Option<(usize, usize, u8)>,
    pub player_word: Option<(usize, usize, u32)>,
    pub manager_byte: Option<(usize, u8)>,
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn signed_word(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn income(bytes: &[u8]) -> i32 {
    signed_word(bytes, 0x1ac)
        .wrapping_add(signed_word(bytes, 0x1b0))
        .max(0)
}

/// Compute the native clamped totals and normalized FTOL results. This is
/// separated from manager-state effects so callers can bind callbacks without
/// moving or copying their player records.
pub fn compute(players: &[Option<crate::player_manager::Player>]) -> Result<ResultState, String> {
    compute_with_prior(players, [0.0; PLAYER_SLOTS])
}

/// Compute using the six f32 words already resident in the native stack
/// locals. Original inactive/null slots skip the normalization write and the
/// later extrema pass still scans those retained words, so callers that do
/// not own a zero-initialized stack must provide the prior values explicitly.
pub fn compute_with_prior(
    players: &[Option<crate::player_manager::Player>],
    prior: [f32; PLAYER_SLOTS],
) -> Result<ResultState, String> {
    if players.len() > PLAYER_SLOTS {
        return Err("player-share roster exceeds six native slots".into());
    }
    if prior.iter().any(|value| !value.is_finite()) {
        return Err("player-share prior stack contains nonfinite f32".into());
    }
    let mut shares = [0i32; PLAYER_SLOTS];
    let mut slot_words = prior;
    let mut stored_values = [0.0_f32; PLAYER_SLOTS];
    let mut active_slots = [false; PLAYER_SLOTS];
    let mut total = 0_i32;
    for (slot, player) in players.iter().enumerate() {
        let Some(player) = player else { continue };
        if player.bytes.get(0x10).copied().unwrap_or(0) == 0 {
            continue;
        }
        active_slots[slot] = true;
        let raw = income(&player.bytes);
        // Native FILD produces the exact signed i32 in PC64, then FST stores
        // that value to the local f32 slot before it is added to the running
        // total. Keep this narrowing step visible rather than multiplying the
        // exact integer directly.
        let stored = Sum::from_i32(raw).to_f32();
        // Native FCOMP against the zero global discards nonpositive values;
        // this explicit comparison also handles signed zero consistently.
        let clamped = if stored > 0.0 { stored } else { 0.0 };
        stored_values[slot] = clamped;
        // 489a36 reloads the running i32 total on every iteration, adds the
        // current f32-stored value in x87, then immediately calls FTOL. This
        // preserves native intermediate rounding and signed wrap behavior.
        let mut running = Sum::from_i32(total);
        running
            .add_f32(clamped)
            .map_err(|_| "player-share total became nonfinite")?;
        total = running.ftol_low_word();
    }
    let mut normalized = [0.0; PLAYER_SLOTS];
    for slot in 0..PLAYER_SLOTS {
        if players
            .get(slot)
            .and_then(Option::as_ref)
            .is_none_or(|p| p.bytes[0x10] == 0)
        {
            continue;
        }
        if total != 0 {
            let mut value = Sum::from_f32(stored_values[slot])
                .map_err(|_| "player-share income became nonfinite")?;
            value
                .mul_f32(10_000.0)
                .map_err(|_| "share scale is nonfinite")?;
            value
                .div(Sum::from_i32(total))
                .map_err(|_| "player-share total is zero")?;
            value.add(Sum::from_f64(0.5).map_err(|_| "share bias is nonfinite")?);
            let rounded = value.ftol_low_word();
            shares[slot] = rounded;
            // 0048a7f first FSTs the FTOL result to f32, then 0048afd
            // reloads that narrowed value before multiplying by 0.01.
            let narrowed = Sum::from_i32(rounded).to_f32();
            slot_words[slot] = narrowed;
            let mut result = Sum::from_f32(narrowed)
                .map_err(|_| "normalized share intermediate is nonfinite")?;
            result
                .mul_f32(0.01)
                .map_err(|_| "normalized share is nonfinite")?;
            normalized[slot] = result.to_f32();
        }
        // Native _ftol returns low EAX=0 for the 0/0 NaN conversion, then
        // FILD/FST leaves a zero f32 word in the slot.
        if total == 0 {
            slot_words[slot] = 0.0;
        }
    }
    let mut active_max = i32::MIN;
    let mut active_min = i32::MAX;
    for (slot, &active) in active_slots.iter().enumerate() {
        if !active {
            continue;
        }
        let value = Sum::from_f32(slot_words[slot])?;
        if Sum::from_i32(active_max).compare(value) == std::cmp::Ordering::Less {
            active_max = value.ftol_low_word();
        }
        if Sum::from_i32(active_min).compare(value) == std::cmp::Ordering::Greater {
            active_min = value.ftol_low_word();
        }
    }
    let mut max_ties = 0;
    let mut max_index = 0;
    let mut min_ties = 0;
    let mut min_index = 0;
    for slot in 0..PLAYER_SLOTS {
        let mut value = Sum::from_f32(slot_words[slot])?;
        if value.compare(Sum::from_i32(active_max)) == std::cmp::Ordering::Equal {
            max_ties += 1;
            max_index = slot;
        }
        if value.compare(Sum::from_i32(active_min)) == std::cmp::Ordering::Equal {
            min_ties += 1;
            min_index = slot;
        }
        value.mul_f32(0.01)?;
        normalized[slot] = value.to_f32();
    }
    Ok(ResultState {
        shares,
        normalized,
        total,
        max_ties,
        max_index,
        min_ties,
        min_index,
    })
}

/// Apply the stateful part of004899e0. The callback receives all downstream
/// calls in native order; `apply_with_callback` is available when a host call
/// must mutate the retained manager/player records before later native reads.
pub fn apply(
    controls: Controls,
    players: &mut [Option<crate::player_manager::Player>],
    mut callback: impl FnMut(Call) -> Result<(), String>,
) -> Result<ResultState, String> {
    apply_with_callback_and_prior(controls, None, [0.0; PLAYER_SLOTS], players, |call| {
        callback(call).map(|_| CallbackEffect::default())
    })
}

/// Variant used by the native comparison harness and by a future manager
/// owner. The late 00489df0 branch writes one controller marker into the
/// 004899e0 manager record; callers that retain that record pass it here.
pub fn apply_with_manager(
    controls: Controls,
    manager_record: Option<&mut [u8]>,
    players: &mut [Option<crate::player_manager::Player>],
    mut callback: impl FnMut(Call) -> Result<(), String>,
) -> Result<ResultState, String> {
    apply_with_callback_and_prior(
        controls,
        manager_record,
        [0.0; PLAYER_SLOTS],
        players,
        |call| callback(call).map(|_| CallbackEffect::default()),
    )
}

/// Full callback form. Returned effects are applied immediately after each
/// call, so subsequent native gate reads observe callback mutations.
pub fn apply_with_callback(
    controls: Controls,
    manager_record: Option<&mut [u8]>,
    players: &mut [Option<crate::player_manager::Player>],
    callback: impl FnMut(Call) -> Result<CallbackEffect, String>,
) -> Result<ResultState, String> {
    apply_with_callback_and_prior(
        controls,
        manager_record,
        [0.0; PLAYER_SLOTS],
        players,
        callback,
    )
}

/// Full callback form with explicit retained f32 local words.
pub fn apply_with_callback_and_prior(
    controls: Controls,
    manager_record: Option<&mut [u8]>,
    prior: [f32; PLAYER_SLOTS],
    players: &mut [Option<crate::player_manager::Player>],
    mut callback: impl FnMut(Call) -> Result<CallbackEffect, String>,
) -> Result<ResultState, String> {
    apply_with_state_callback(controls, manager_record, prior, players, |call, _, _, _| {
        callback(call)
    })
}

/// Retained callbacks may execute downstream simulation helpers synchronously.
/// Later reads observe their writes, including writes made before an error.
pub fn apply_with_state_callback(
    mut controls: Controls,
    mut manager_record: Option<&mut [u8]>,
    prior: [f32; PLAYER_SLOTS],
    players: &mut [Option<crate::player_manager::Player>],
    mut callback: impl FnMut(
        Call,
        &mut Controls,
        Option<&mut [u8]>,
        &mut [Option<crate::player_manager::Player>],
    ) -> Result<CallbackEffect, String>,
) -> Result<ResultState, String> {
    fn dispatch(
        callback: &mut impl FnMut(
            Call,
            &mut Controls,
            Option<&mut [u8]>,
            &mut [Option<crate::player_manager::Player>],
        ) -> Result<CallbackEffect, String>,
        call: Call,
        controls: &mut Controls,
        manager_record: &mut Option<&mut [u8]>,
        players: &mut [Option<crate::player_manager::Player>],
    ) -> Result<CallbackEffect, String> {
        let effect = callback(call, controls, manager_record.as_deref_mut(), players)?;
        if let Some(value) = effect.manager_value {
            controls.manager_value = value;
            if let Some(record) = manager_record.as_deref_mut() {
                record
                    .get_mut(4..8)
                    .ok_or("market-share target record truncated")?
                    .copy_from_slice(&value.to_le_bytes());
            }
        }
        if let Some(value) = effect.network_enabled {
            controls.network_enabled = value;
        }
        if let Some(value) = effect.network_override {
            controls.network_override = value;
        }
        if let Some(value) = effect.current_slot {
            controls.current_slot = value;
        }
        if let Some((slot, value)) = effect.player_auxiliary {
            let player = players
                .get_mut(slot)
                .and_then(Option::as_mut)
                .ok_or_else(|| format!("callback auxiliary slot {slot} is not allocated"))?;
            player.bytes[0x14c..0x150].copy_from_slice(&value.to_le_bytes());
        }
        if let Some((slot, offset, value)) = effect.player_byte {
            let player = players
                .get_mut(slot)
                .and_then(Option::as_mut)
                .ok_or_else(|| format!("callback byte slot {slot} is not allocated"))?;
            let byte = player
                .bytes
                .get_mut(offset)
                .ok_or("callback player byte outside record")?;
            *byte = value;
        }
        if let Some((slot, offset, value)) = effect.player_word {
            let player = players
                .get_mut(slot)
                .and_then(Option::as_mut)
                .ok_or_else(|| format!("callback word slot {slot} is not allocated"))?;
            let end = offset
                .checked_add(4)
                .ok_or("callback player word overflow")?;
            let bytes = player
                .bytes
                .get_mut(offset..end)
                .ok_or("callback player word outside record")?;
            bytes.copy_from_slice(&value.to_le_bytes());
        }
        if let Some((offset, value)) = effect.manager_byte {
            let manager = manager_record
                .as_deref_mut()
                .ok_or("callback requested manager write without manager record")?;
            if offset >= manager.len() {
                return Err("callback manager write outside record".into());
            }
            manager[offset] = value;
        }
        Ok(effect)
    }
    if players.len() > PLAYER_SLOTS {
        return Err("player-share roster exceeds six native slots".into());
    }
    let result = compute_with_prior(players, prior)?;
    if controls.player_count > PLAYER_SLOTS {
        return Err("player-share player count exceeds six native slots".into());
    }
    if controls.current_slot >= controls.player_count && controls.current_slot < PLAYER_SLOTS {
        return Err("player-share current slot is outside counted roster".into());
    }
    let manager_target = Sum::from_f32(controls.manager_value)
        .map_err(|_| "player-share manager value is nonfinite")?
        .ftol_low_word();
    if controls.manager_kind == 2 && manager_target.wrapping_sub(1) as u32 == controls.day {
        // The native winner notification distinguishes a tied minimum from
        // the current player, the last maximum, and the remaining case.
        let (message, detail) = if result.min_ties > 1 {
            (0x6b5, 0x6bb)
        } else {
            let current_marker = players
                .get(controls.current_slot)
                .and_then(Option::as_ref)
                .map(|p| p.bytes[0x1bc] as usize)
                .unwrap_or(usize::MAX);
            let detail = if current_marker == result.max_index {
                0x6bc
            } else if current_marker == result.min_index {
                0x6bd
            } else {
                0x6be
            };
            (0x6b5, detail)
        };
        dispatch(
            &mut callback,
            Call::Objective { message, detail },
            &mut controls,
            &mut manager_record,
            players,
        )?;
    }
    // 489bb5 skips the kind2 profile/removal pass while signed day < target.
    // Both that path and the preceding-day notice still reach common cleanup.
    // Kind1 profiles only beyond its special day-one notification branch.
    if controls.manager_kind == 2 && manager_target.wrapping_sub(1) as u32 == controls.day {
        // The target-match notification above returns directly to cleanup.
    } else if (controls.manager_kind == 2 && controls.day as i32 >= manager_target)
        || (controls.manager_kind == 1 && controls.day as i32 > 1)
    {
        dispatch(
            &mut callback,
            Call::Refresh,
            &mut controls,
            &mut manager_record,
            players,
        )?;
        if result.min_ties == 1 && controls.manager_kind == 2 {
            dispatch(
                &mut callback,
                Call::ProfileSelect {
                    index: result.min_index as u32,
                },
                &mut controls,
                &mut manager_record,
                players,
            )?;
        } else if result.max_ties == 1 && controls.manager_kind == 1 {
            let slot = result.max_index;
            let player = players[slot]
                .as_mut()
                .ok_or_else(|| format!("player-share max winner slot {slot} is not allocated"))?;
            let aux = word(&player.bytes, 0x14c).wrapping_add(1);
            player.bytes[0x14c..0x150].copy_from_slice(&aux.to_le_bytes());
            let _ = dispatch(
                &mut callback,
                Call::Kind1ProfileSelect { index: slot as u32 },
                &mut controls,
                &mut manager_record,
                players,
            )?;
        }
        for slot in 0..players.len() {
            let Some(player) = players[slot].as_ref() else {
                continue;
            };
            if player.bytes[0x10] == 0 {
                continue;
            }
            let normalized = result.normalized[slot];
            let auxiliary_reference = word(&player.bytes, 0x14c);
            dispatch(
                &mut callback,
                Call::PlayerShare {
                    slot,
                    normalized,
                    auxiliary_reference,
                },
                &mut controls,
                &mut manager_record,
                players,
            )?;
        }
        if controls.manager_kind == 2 && result.min_ties == 1 {
            if !controls.network_enabled || controls.network_override {
                dispatch(
                    &mut callback,
                    Call::CommandProducer {
                        slot: result.min_index as u32,
                    },
                    &mut controls,
                    &mut manager_record,
                    players,
                )?;
            }
            // 0048c5e rereads both gate bytes after 00450490 returns.
            if !controls.network_enabled || controls.network_override {
                dispatch(
                    &mut callback,
                    Call::Command {
                        args: [
                            result.min_index as u32,
                            u32::MAX,
                            0x2d,
                            0,
                            0,
                            0,
                            0,
                            0xffff_fc19,
                        ],
                    },
                    &mut controls,
                    &mut manager_record,
                    players,
                )?;
                dispatch(
                    &mut callback,
                    Call::RemovePlayer {
                        slot: result.min_index as u32,
                    },
                    &mut controls,
                    &mut manager_record,
                    players,
                )?;
            }
        }
        if controls.manager_kind == 1 && result.max_ties == 1 {
            let slot = result.max_index;
            let aux = word(&players[slot].as_ref().unwrap().bytes, 0x14c);
            let manager_target_after = Sum::from_f32(controls.manager_value)
                .map_err(|_| "player-share manager value became nonfinite")?
                .ftol_low_word();
            if manager_target_after as u32 == aux {
                //489dad stores the marker before calling489df0.
                let offset = 0x0d + slot * 8;
                if let Some(manager) = manager_record.as_deref_mut() {
                    if offset >= manager.len() {
                        return Err("player-share manager controller marker outside record".into());
                    }
                    manager[offset] = 1;
                }
                dispatch(
                    &mut callback,
                    Call::LateController { argument: 1 },
                    &mut controls,
                    &mut manager_record,
                    players,
                )?;
            }
        }
    } else if controls.manager_kind == 1 && (controls.day as i32) < 1 {
        //489ca6 jumps past the reset: retain first-day accumulated income.
        return Ok(result);
    } else if controls.manager_kind == 1 && controls.day == 1 {
        let detail = if result.max_ties > 1 {
            0x6b7
        } else {
            let current_marker = players
                .get(controls.current_slot)
                .and_then(Option::as_ref)
                .map(|p| p.bytes[0x1bc] as usize)
                .unwrap_or(usize::MAX);
            if current_marker == result.max_index {
                0x6b8
            } else if current_marker == result.min_index {
                0x6b9
            } else {
                0x6ba
            }
        };
        dispatch(
            &mut callback,
            Call::Objective {
                message: 0x6b4,
                detail,
            },
            &mut controls,
            &mut manager_record,
            players,
        )?;
    } else if controls.manager_kind != 0 {
        // All other manager kinds take the original common reset exit.
    }
    for (slot, player) in players.iter_mut().enumerate().take(controls.player_count) {
        if player.is_none() {
            return Err(format!("player-share null pointer in counted slot {slot}"));
        }
        if let Some(player) = player {
            player.bytes[0x1ac..0x1b0].fill(0);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_manager::Player;

    #[test]
    fn weakest_link_waits_until_authored_start_day() {
        #[derive(Clone, Copy)]
        enum Phase {
            Quiet,
            Notice,
            Removal,
        }
        // Original 489bb5's signed comparison skips profiles and removal
        // before the target. The preceding day has only its notice callback.
        for (target, day, phase) in [
            (3., 0, Phase::Quiet),
            (3., 1, Phase::Quiet),
            (3., 2, Phase::Notice),
            (3., 3, Phase::Removal),
            (3., 4, Phase::Removal),
            (5., 1, Phase::Quiet),
            (5., 4, Phase::Notice),
            (5., 5, Phase::Removal),
            (3., u32::MAX, Phase::Quiet),
            (3.75, 2, Phase::Notice),
            (3.75, 3, Phase::Removal),
        ] {
            let mut players: Vec<_> = (0..PLAYER_SLOTS)
                .map(|slot| {
                    let mut bytes = vec![0; 0x220];
                    bytes[0x10] = 1;
                    bytes[0x1bc] = slot as u8;
                    bytes[0x1ac..0x1b0].copy_from_slice(&((slot as i32 + 1) * 10).to_le_bytes());
                    Some(Player {
                        reference: 0x10000000 + slot as u32 * 0x1000,
                        bytes,
                        auxiliaries: Vec::new(),
                    })
                })
                .collect();
            let mut calls = Vec::new();
            let result = apply_with_state_callback(
                Controls {
                    manager_kind: 2,
                    manager_value: target,
                    day,
                    player_count: PLAYER_SLOTS,
                    network_enabled: false,
                    network_override: false,
                    current_slot: 0,
                },
                None,
                [0.; PLAYER_SLOTS],
                &mut players,
                |call, _, _, _| {
                    calls.push(call);
                    Ok(CallbackEffect::default())
                },
            )
            .unwrap();
            assert_eq!(result.min_index, 0);
            match phase {
                Phase::Quiet => assert!(
                    calls.is_empty(),
                    "before target {target}, day {day}: {calls:?}"
                ),
                Phase::Notice => assert!(matches!(
                    calls.as_slice(),
                    [Call::Objective {
                        message: 0x6b5,
                        detail: 0x6bd
                    }]
                )),
                Phase::Removal => {
                    assert!(matches!(calls.first(), Some(Call::Refresh)));
                    assert!(
                        calls
                            .iter()
                            .any(|call| matches!(call, Call::CommandProducer { slot: 0 }))
                    );
                    assert!(
                        calls
                            .iter()
                            .any(|call| matches!(call, Call::RemovePlayer { slot: 0 }))
                    );
                    assert_eq!(
                        calls
                            .iter()
                            .filter(|call| matches!(call, Call::PlayerShare { .. }))
                            .count(),
                        PLAYER_SLOTS
                    );
                }
            }
            for player in players.iter().flatten() {
                assert_eq!(
                    word(&player.bytes, 0x1ac),
                    0,
                    "pre-target exit must still clear accumulated retail counters"
                );
            }
        }
    }

    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
    }
    impl Reader<'_> {
        fn take<const N: usize>(&mut self) -> [u8; N] {
            let end = self.at + N;
            let value = self.bytes[self.at..end].try_into().unwrap();
            self.at = end;
            value
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take())
        }
    }

    #[test]
    #[ignore = "requires OPENMT2001_NATIVE_PLAYER_SHARE"]
    fn matches_native_player_share() {
        let path = crate::fixtures::path("NATIVE_PLAYER_SHARE");
        let bytes = std::fs::read(path).unwrap();
        let mut reader = Reader {
            bytes: &bytes,
            at: 0,
        };
        assert_eq!(reader.take::<8>(), *b"MTPSHR03");
        assert_eq!(reader.word(), 3);
        let count = reader.word() as usize;
        for case in 0..count {
            let manager_kind = reader.word() as i32;
            let _manager_value = reader.word();
            let _day = reader.word();
            let _player_count = reader.word();
            let network = reader.word() != 0;
            let override_gate = reader.word() != 0;
            let current_slot = reader.word() as usize;
            for _ in 0..PLAYER_SLOTS {
                let _retail = reader.word();
                let _housing = reader.word();
            }
            let _active = reader.take::<PLAYER_SLOTS>();
            let prior = std::array::from_fn(|_| f32::from_bits(reader.word()));
            let trace_count = reader.word() as usize;
            let mut native_trace = Vec::new();
            let mut native_managers = Vec::new();
            for _ in 0..trace_count {
                let address = reader.word();
                let _ecx = reader.word();
                let argc = reader.word() as usize;
                let args: Vec<_> = (0..argc).map(|_| reader.word()).collect();
                native_trace.push((address, args));
                native_managers.push(reader.take::<60>());
            }
            let manager_before = reader.take::<0x3c>();
            let before: Vec<[u8; 0x220]> = (0..PLAYER_SLOTS).map(|_| reader.take()).collect();
            let manager_after = reader.take::<0x3c>();
            let after: Vec<[u8; 0x220]> = (0..PLAYER_SLOTS).map(|_| reader.take()).collect();
            let mut players: Vec<Option<Player>> = before
                .iter()
                .enumerate()
                .map(|(slot, bytes)| {
                    if slot >= _player_count as usize {
                        return None;
                    }
                    Some(Player {
                        reference: 0,
                        bytes: bytes.to_vec(),
                        auxiliaries: Vec::new(),
                    })
                })
                .collect();
            let mut manager_runtime = manager_before.to_vec();
            let mut calls = Vec::new();
            let result = apply_with_state_callback(
                Controls {
                    manager_kind,
                    manager_value: f32::from_bits(_manager_value),
                    day: _day,
                    player_count: _player_count as usize,
                    network_enabled: network,
                    network_override: override_gate,
                    current_slot,
                },
                Some(&mut manager_runtime),
                prior,
                &mut players,
                |call, _, manager, _| {
                    assert_eq!(
                        manager.unwrap(),
                        native_managers[calls.len()],
                        "case {case} callback {}",
                        calls.len()
                    );
                    calls.push(call);
                    Ok(CallbackEffect::default())
                },
            )
            .unwrap();
            assert_eq!(
                manager_runtime, manager_after,
                "native manager writes case {case}"
            );
            for (slot, player) in players.iter().enumerate() {
                if slot < _player_count as usize {
                    assert_eq!(
                        player.as_ref().unwrap().bytes,
                        after[slot],
                        "native player-share case {case} slot {slot}"
                    );
                } else {
                    assert!(player.is_none());
                    assert_eq!(
                        before[slot], after[slot],
                        "native player-share uncounted slot {case} slot {slot}"
                    );
                }
            }
            let rust_trace: Vec<_> = calls
                .iter()
                .map(|call| match call {
                    Call::Objective { message, detail } => (0x0048a150, vec![*message, *detail]),
                    Call::Refresh => (0x0043be20, vec![]),
                    Call::PlayerShare {
                        slot,
                        normalized,
                        auxiliary_reference,
                    } => (
                        0x0042c4b0,
                        vec![*slot as u32, normalized.to_bits(), *auxiliary_reference],
                    ),
                    Call::ProfileSelect { index } => (0x0042c470, vec![*index]),
                    Call::Kind1ProfileSelect { index } => (0x0042c430, vec![*index]),
                    Call::Command { args } => (0x0048cab0, args.to_vec()),
                    Call::CommandProducer { slot } => (0x00450490, vec![*slot]),
                    Call::RemovePlayer { slot } => (0x0048cc50, vec![*slot]),
                    Call::LateController { argument } => (0x00489df0, vec![*argument]),
                })
                .collect();
            assert_eq!(rust_trace, native_trace, "native player-share case {case}");
            assert_eq!(result.shares.len(), PLAYER_SLOTS);
        }
        assert_eq!(reader.at, bytes.len());
    }
}

/// Test observation of the Session's 18:00 market-share pass (helper 4899e0).
/// A test registers hooks for its own thread; nothing sets them in the game.
#[cfg(feature = "lua4")]
pub mod observer {
    use crate::{level_startup::City, player_manager::Manager};
    use std::cell::Cell;

    #[derive(Clone, Copy)]
    pub struct Hooks {
        /// manager, city, current player reference, day, prior shares, network flags.
        pub before: fn(&Manager, &City, u32, u32, [f32; 6], [bool; 2]),
        pub after: fn(&Manager, &City),
    }
    thread_local! { static HOOKS: Cell<Option<Hooks>> = const { Cell::new(None) }; }

    pub fn set(hooks: Option<Hooks>) {
        HOOKS.with(|h| h.set(hooks));
    }
    pub fn before(
        manager: &Manager,
        city: &City,
        current: u32,
        day: u32,
        prior: [f32; 6],
        network: [bool; 2],
    ) {
        if let Some(hooks) = HOOKS.with(Cell::get) {
            (hooks.before)(manager, city, current, day, prior, network);
        }
    }
    pub fn after(manager: &Manager, city: &City) {
        if let Some(hooks) = HOOKS.with(Cell::get) {
            (hooks.after)(manager, city);
        }
    }
}
