//! Original467440 primary AI event consumption. Queue selection is caller-owned.

const PROFILE_BASE: u32 = 0x7f92f8;

fn live_record(profiles: &[[u8; 0x650]], reference: u32) -> Result<Record, String> {
    let offset = reference
        .checked_sub(PROFILE_BASE)
        .ok_or("AI event reference precedes profiles")? as usize;
    let profile = profiles
        .get(offset / 0x650)
        .ok_or("AI event reference leaves profiles")?;
    let bytes = profile
        .get(offset % 0x650..offset % 0x650 + 8)
        .ok_or("AI event crosses profile boundary")?;
    Ok(Record {
        timestamp: i32::from_le_bytes(bytes[..4].try_into().unwrap()),
        code: i32::from_le_bytes(bytes[4..].try_into().unwrap()),
    })
}

fn live_queue(
    player: &mut crate::player_manager::Player,
    profiles: &mut [[u8; 0x650]],
    clock: i32,
    secondary: bool,
    advance: bool,
) -> Result<u32, String> {
    use crate::player_manager::{put, word};
    let character = word(&player.bytes, 0x150) as usize;
    let profile = profiles
        .get_mut(character)
        .ok_or("AI timetable character leaves profiles")?;
    let enable = if secondary { 0x23a } else { 0x239 };
    let cursor = if secondary { 0x18 } else { 0x14 };
    let base = if secondary { 0x280 } else { 0x410 };
    let mut queue = if secondary {
        Timetable::secondary_from_profile(profile, player.bytes[enable])
    } else {
        Timetable::from_profile(profile, player.bytes[enable])
    };
    let position = if advance {
        queue.advance()?;
        queue.cursor as usize
    } else {
        let mut primary_cursor = word(profile, 0x14) as i32;
        if secondary {
            queue.select_secondary(clock, &mut primary_cursor)?;
            put(profile, 0x14, primary_cursor as u32);
        } else {
            queue.select(clock)?;
        }
        queue
            .records
            .iter()
            .take(queue.count.max(0) as usize)
            .position(|r| r.timestamp >= clock)
            .unwrap_or(0)
    };
    put(profile, cursor, queue.cursor as u32);
    player.bytes[enable] = queue.enabled;
    Ok(PROFILE_BASE + (character * 0x650 + base + position * 8) as u32)
}

///4781a0 over the sole-owned player/profile records, preserving native pointers.
pub fn poll_player(
    player: &mut crate::player_manager::Player,
    profiles: &mut [[u8; 0x650]],
    clock: i32,
) -> Result<(), String> {
    use crate::player_manager::{put, word};
    if player.bytes.len() < 0x1e50 {
        return Err("truncated AI timetable player".into());
    }
    if player.bytes[0x1401] == 0 || player.bytes[0x1df8] != 0 {
        return Ok(());
    }
    for (secondary, pointer, enable, pending) in [
        (false, 0x1e1c, 0x239, 0x1e11),
        (true, 0x1e20, 0x23a, 0x1e12),
    ] {
        if word(&player.bytes, pointer) == 0 {
            let reference = live_queue(player, profiles, clock, secondary, false)?;
            put(&mut player.bytes, pointer, reference);
        }
        if player.bytes[enable] != 0
            && clock >= live_record(profiles, word(&player.bytes, pointer))?.timestamp
        {
            player.bytes[pending] = 1;
        }
    }
    Ok(())
}

///477f20's manager order, including the midnight enable-byte refresh.
pub fn poll_players(
    players: &mut [crate::player_manager::Player],
    profiles: &mut [[u8; 0x650]],
    clock: i32,
) -> Result<(), String> {
    for player in players {
        if player.bytes[0x10] == 0 || crate::player_manager::word(&player.bytes, 0x154) != 1 {
            continue;
        }
        if clock == 0 {
            player.bytes[0x238..0x23b].fill(1);
        }
        poll_player(player, profiles, clock)?;
    }
    Ok(())
}

///467370/467440 consume against live profile pointers. Diagnostics remain
/// synchronous; queue advancement reloads profile identity after the callback.
pub fn consume_player(
    player: &mut crate::player_manager::Player,
    profiles: &mut [[u8; 0x650]],
    clock: i32,
    secondary: bool,
    category_locks: &[u8; 52],
    mut log: impl FnMut(
        crate::ai_diagnostic::Diagnostic,
        &mut crate::player_manager::Player,
        &mut [[u8; 0x650]],
    ) -> Result<(), String>,
) -> Result<(), String> {
    use crate::player_manager::{put, word};
    let (pointer, pending) = if secondary {
        (0x1e20, 0x1e12)
    } else {
        (0x1e1c, 0x1e11)
    };
    if player.bytes[pending] == 0 {
        return Ok(());
    }
    let event = live_record(profiles, word(&player.bytes, pointer))?;
    if clock >= event.timestamp
        && event.code != -1
        && player.bytes[0x1402] == 0
        && !matches!(word(&player.bytes, 0x240), 2 | 4 | 7)
    {
        let category = word(&player.bytes, 0xb68) as i32;
        let rejected = category >= 0
            && *category_locks
                .get(category as usize)
                .ok_or("AI event category leaves table")?
                != 0;
        if !rejected {
            if secondary {
                if !matches!(event.code, 0 | 7) {
                    log(
                        crate::ai_diagnostic::Diagnostic::timetable(secondary, event.code),
                        player,
                        profiles,
                    )?;
                }
            } else if let Some(bit) = event_bit(event.code) {
                let flags = word(&player.bytes, 0x220) | bit;
                put(&mut player.bytes, 0x220, flags);
            } else {
                log(
                    crate::ai_diagnostic::Diagnostic::timetable(secondary, event.code),
                    player,
                    profiles,
                )?;
            }
        }
    }
    let reference = live_queue(player, profiles, clock, secondary, true)?;
    put(&mut player.bytes, pointer, reference);
    player.bytes[pending] = 0;
    Ok(())
}

fn event_bit(code: i32) -> Option<u32> {
    Some(match code {
        0 => 0,
        1 => 1,
        4 => 4,
        5 => 0x2000,
        11 => 0x200,
        12 => 8,
        13 => 0x100,
        14 => 0x20,
        _ => return None,
    })
}

/// Original Lua LoadTimeTable/ResetTimeTable binding after argument extraction.
/// This preserves the original profile bytes, including padding and other tables.
pub fn configure_timetable(
    profile: &mut [u8; 0x650],
    reset: bool,
    arguments: [f64; 4],
) -> Result<(), String> {
    let [kind, hour, minute, code] = arguments.map(crate::economy::ftol_low_word);
    let (cursor, count_at, records) = match kind {
        0 => (0x10, 0x1c, 0xf0),
        1 => (0x14, 0x20, 0x410),
        2 => (0x18, 0x24, 0x280),
        _ => return Ok(()),
    };
    if reset {
        profile[cursor..cursor + 4].fill(0);
        profile[count_at..count_at + 4].fill(0);
        profile[records..records + 400].fill(0xff);
    } else {
        let count = i32::from_le_bytes(profile[count_at..count_at + 4].try_into().unwrap());
        if count < 50 {
            let offset = (records as i64 + i64::from(count) * 8) as usize;
            let record = profile
                .get_mut(offset..offset.saturating_add(8))
                .ok_or("original timetable write leaves profile")?;
            record[..4].copy_from_slice(
                &hour
                    .wrapping_mul(625)
                    .wrapping_add(minute.wrapping_mul(10))
                    .to_le_bytes(),
            );
            record[4..].copy_from_slice(&code.to_le_bytes());
            profile[count_at..count_at + 4].copy_from_slice(&count.wrapping_add(1).to_le_bytes());
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub timestamp: i32,
    pub code: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub pending: u8,
    pub event: Option<Record>,
    pub flags: u32,
    pub disabled: u8,
    pub mode: i32,
    pub category: i32,
}

/// One original profile's primary queue. Count and cursor retain their signed
/// original semantics. Storage population and capacity validation are separate.
pub struct Timetable {
    pub records: Vec<Record>,
    pub count: i32,
    pub cursor: i32,
    pub enabled: u8,
}

impl Timetable {
    /// Transfer the primary timetable loaded into an original-format profile.
    pub fn from_profile(profile: &[u8; 0x650], enabled: u8) -> Self {
        let word = |at| i32::from_le_bytes(profile[at..at + 4].try_into().unwrap());
        Self {
            records: (0..50)
                .map(|i| Record {
                    timestamp: word(0x410 + i * 8),
                    code: word(0x414 + i * 8),
                })
                .collect(),
            count: word(0x20),
            cursor: word(0x14),
            enabled,
        }
    }

    pub fn secondary_from_profile(profile: &[u8; 0x650], enabled: u8) -> Self {
        let word = |at| i32::from_le_bytes(profile[at..at + 4].try_into().unwrap());
        Self {
            records: (0..50)
                .map(|i| Record {
                    timestamp: word(0x280 + i * 8),
                    code: word(0x284 + i * 8),
                })
                .collect(),
            count: word(0x24),
            cursor: word(0x18),
            enabled,
        }
    }

    /// Original477f80 has a cross-table exhaustion side effect: it resets the
    /// primary cursor, while preserving the secondary cursor's skipped count.
    pub fn select_secondary(
        &mut self,
        clock: i32,
        primary_cursor: &mut i32,
    ) -> Result<Record, String> {
        for index in 0..self.count.max(0) as usize {
            let record = *self
                .records
                .get(index)
                .ok_or("AI secondary queue leaves storage")?;
            if record.timestamp >= clock {
                return Ok(record);
            }
            self.cursor = self.cursor.wrapping_add(1);
        }
        *primary_cursor = 0;
        self.enabled = 0;
        self.records
            .first()
            .copied()
            .ok_or("AI secondary queue has no base record".into())
    }

    /// Original478090 scans from record zero but increments the existing cursor
    /// for skipped records. It does not first reset that cursor.
    pub fn select(&mut self, clock: i32) -> Result<Record, String> {
        for index in 0..self.count.max(0) as usize {
            let record = *self
                .records
                .get(index)
                .ok_or("AI primary queue leaves storage")?;
            if record.timestamp >= clock {
                return Ok(record);
            }
            self.cursor = self.cursor.wrapping_add(1);
        }
        self.cursor = 0;
        self.enabled = 0;
        self.records
            .first()
            .copied()
            .ok_or("AI primary queue has no base record".into())
    }

    /// Original478130 returns record zero even on exhaustion; the separate
    /// enable byte prevents subsequent scheduling, rather than a null pointer.
    pub fn advance(&mut self) -> Result<Record, String> {
        self.cursor = self.cursor.wrapping_add(1);
        if self.cursor >= self.count {
            self.cursor = 0;
            self.enabled = 0;
        }
        self.records
            .get(self.cursor as usize)
            .copied()
            .ok_or("AI primary queue leaves storage".into())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingEvent {
    pub pending: u8,
    pub event: Option<Record>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScheduledEvents {
    pub active: u8,
    pub blocked: u8,
    pub primary: State,
    pub secondary: PendingEvent,
}

/// Original4781a0. Missing event pointers are selected before enable checks;
/// existing pending bytes survive every non-due or disabled path.
pub fn poll(
    state: &mut ScheduledEvents,
    primary: &mut Timetable,
    secondary: &mut Timetable,
    clock: i32,
) -> Result<(), String> {
    if state.active == 0 || state.blocked != 0 {
        return Ok(());
    }
    if state.primary.event.is_none() {
        state.primary.event = Some(primary.select(clock)?);
    }
    if primary.enabled != 0
        && state
            .primary
            .event
            .is_some_and(|event| clock >= event.timestamp)
    {
        state.primary.pending = 1;
    }
    if state.secondary.event.is_none() {
        state.secondary.event = Some(secondary.select_secondary(clock, &mut primary.cursor)?);
    }
    if secondary.enabled != 0
        && state
            .secondary
            .event
            .is_some_and(|event| clock >= event.timestamp)
    {
        state.secondary.pending = 1;
    }
    Ok(())
}

/// Original467370 consumes the secondary timetable before primary decisions.
/// The original recognizes code7 but performs no business action for it here.
pub fn consume_secondary(
    state: &mut ScheduledEvents,
    queue: &mut Timetable,
    clock: i32,
    category_bytes: &[u8],
    mut log_unknown: impl FnMut(i32, &mut ScheduledEvents) -> Result<(), String>,
) -> Result<(), String> {
    if state.secondary.pending == 0 {
        return Ok(());
    }
    let event = state
        .secondary
        .event
        .ok_or("original pending secondary event dereferences null")?;
    if clock >= event.timestamp
        && event.code != -1
        && state.primary.disabled == 0
        && !matches!(state.primary.mode, 2 | 4 | 7)
    {
        let rejected = if state.primary.category >= 0 {
            *category_bytes
                .get(state.primary.category as usize)
                .ok_or("AI secondary category leaves table")?
                != 0
        } else {
            false
        };
        if !rejected && !matches!(event.code, 0 | 7) {
            log_unknown(event.code, state)?;
        }
    }
    state.secondary.event = Some(queue.advance()?);
    state.secondary.pending = 0;
    Ok(())
}

/// Consume one pending event, including the original future-event advancement.
/// Category bytes correspond to original617694 + category*380. The callbacks
/// expose logging and478130 queue advancement without inventing queue contents.
pub fn consume(
    state: &mut State,
    clock: i32,
    category_bytes: &[u8],
    mut log_unknown: impl FnMut(i32, &mut State) -> Result<(), String>,
    mut next: impl FnMut(&mut State) -> Result<Option<Record>, String>,
) -> Result<(), String> {
    if state.pending == 0 {
        return Ok(());
    }
    let event = state
        .event
        .ok_or("original pending AI event dereferences null")?;
    if clock >= event.timestamp
        && event.code != -1
        && state.disabled == 0
        && !matches!(state.mode, 2 | 4 | 7)
    {
        let rejected = if state.category >= 0 {
            *category_bytes
                .get(state.category as usize)
                .ok_or("AI event category leaves table")?
                != 0
        } else {
            false
        };
        if !rejected {
            let bit = if let Some(bit) = event_bit(event.code) {
                bit
            } else {
                log_unknown(event.code, state)?;
                0
            };
            state.flags |= bit;
        }
    }
    state.event = next(state)?;
    state.pending = 0;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original live AI timetable sequences"]
    fn matches_native_live_events() {
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
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_LIVE_EVENTS")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTAILIV1");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let clock = r.word() as i32;
            let rejected = r.word() as u8;
            let mutate = r.word() != 0;
            let mut player = crate::player_manager::Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let mut profiles: Vec<[u8; 0x650]> = r
                .blob()
                .chunks_exact(0x650)
                .map(|b| b.try_into().unwrap())
                .collect();
            let mut categories = [[0; 0x17c]; 52];
            categories[0][0x6c] = rejected;
            let mut trace = vec![];
            for step in 0..3 {
                if step == 0 {
                    poll_player(&mut player, &mut profiles, clock).unwrap();
                } else {
                    consume_player(
                        &mut player,
                        &mut profiles,
                        clock,
                        step == 1,
                        &categories.map(|c| c[0x6c]),
                        |diagnostic, p, _| {
                            let crate::ai_diagnostic::Message::Timetable { code, .. } =
                                diagnostic.message
                            else {
                                panic!("expected timetable diagnostic")
                            };
                            assert_eq!(
                                diagnostic.kind,
                                crate::ai_diagnostic::Kind::RecoverableWarning
                            );
                            trace.push(code as u32);
                            if mutate {
                                crate::player_manager::put(&mut p.bytes, 0x150, 1);
                            }
                            Ok(())
                        },
                    )
                    .unwrap();
                }
                let expected = r.blob();
                for (at, (&a, &b)) in player.bytes.iter().zip(expected).enumerate() {
                    assert_eq!(a, b, "{name} step{step} player+{at:x}");
                }
                let expected = r.blob();
                for (at, (&a, &b)) in profiles.iter().flatten().zip(expected).enumerate() {
                    assert_eq!(a, b, "{name} step{step} profile+{at:x}");
                }
            }
            let expected: Vec<_> = (0..r.word()).map(|_| r.word()).collect();
            assert_eq!(trace, expected, "{name}");
        }
        assert!(r.0.is_empty());
    }

    #[test]
    #[ignore = "requires original secondary event consumption observations"]
    fn matches_native_secondary_consumption() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_SECONDARY_EVENT")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAISEC1");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let cfg: [u32; 15] = std::array::from_fn(|i| word(at + i * 4));
            at += 60;
            let records: Vec<_> = (0..4)
                .map(|i| Record {
                    timestamp: word(at + i * 8) as i32,
                    code: word(at + i * 8 + 4) as i32,
                })
                .collect();
            at += 32;
            let initial: [u32; 11] = std::array::from_fn(|i| word(at + i * 4));
            at += 44;
            let expected: [u32; 11] = std::array::from_fn(|i| word(at + i * 4));
            at += 44;
            let count = word(at) as usize;
            at += 4;
            let logged = (0..count).any(|i| word(at + i * 32) == 0x4e6960);
            let advances = (0..count).filter(|i| word(at + i * 32) == 0x478020).count();
            at += count * 32;
            let event = Record {
                timestamp: cfg[2] as i32,
                code: cfg[3] as i32,
            };
            let mut state = ScheduledEvents {
                primary: State {
                    flags: 0xa55a0000 | cfg[12] | cfg[13] << 8,
                    disabled: cfg[5] as u8,
                    mode: cfg[6] as i32,
                    category: cfg[7] as i32,
                    ..Default::default()
                },
                secondary: PendingEvent {
                    pending: initial[0] as u8,
                    event: (cfg[1] != 0).then_some(event),
                },
                ..Default::default()
            };
            let mut categories = vec![0; (state.primary.category.max(0) + 1) as usize];
            if state.primary.category >= 0 {
                categories[state.primary.category as usize] = cfg[9] as u8;
            }
            let mut queue = Timetable {
                records: records.clone(),
                cursor: initial[5] as i32,
                count: initial[6] as i32,
                enabled: initial[4] as u8,
            };
            let mut actual_log = false;
            let result = consume_secondary(
                &mut state,
                &mut queue,
                cfg[4] as i32,
                &categories,
                |code, _| {
                    assert_eq!(code, event.code);
                    actual_log = true;
                    Ok(())
                },
            );
            assert_eq!(result.is_err(), expected[10] != 0, "secondary{case}");
            assert_eq!(actual_log, logged, "secondary{case}");
            assert_eq!(
                state.primary.flags,
                0xa55a0000 | expected[2] | expected[3] << 8,
                "secondary{case}"
            );
            assert_eq!(
                state.secondary.pending, expected[0] as u8,
                "secondary{case}"
            );
            assert_eq!(
                [
                    u32::from(queue.enabled),
                    queue.cursor as u32,
                    queue.count as u32
                ],
                expected[4..7],
                "secondary{case}"
            );
            let expected_event = if expected[1] == 0 {
                None
            } else if expected[1] == 0x10012000 {
                Some(event)
            } else {
                let base = 0x7f9578_u32.wrapping_add(cfg[8].wrapping_mul(0x650));
                Some(records[(expected[1] - base) as usize / 8])
            };
            assert_eq!(state.secondary.event, expected_event, "secondary{case}");
            assert_eq!(
                advances,
                usize::from(initial[0] != 0 && cfg[1] != 0),
                "secondary{case}"
            );
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original pending-event queue-check observations"]
    fn matches_native_queue_poll() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_QUEUE_CHECK")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAIQCK1");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let cfg: [u32; 16] = std::array::from_fn(|i| word(at + i * 4));
            at += 64;
            let records: [Record; 8] = std::array::from_fn(|i| Record {
                timestamp: word(at + i * 8) as i32,
                code: word(at + i * 8 + 4) as i32,
            });
            at += 64;
            let initial: [u32; 10] = std::array::from_fn(|i| word(at + i * 4));
            at += 40;
            let expected: [u32; 10] = std::array::from_fn(|i| word(at + i * 4));
            at += 40;
            let calls = word(at) as usize;
            at += 4 + calls * 16;
            let resolve = |ptr: u32, secondary: bool| -> Option<Record> {
                if ptr == 0 {
                    return None;
                }
                if ptr == if secondary { 0x10012020 } else { 0x10012000 } {
                    let index = if secondary { 14 } else { 12 };
                    return Some(Record {
                        timestamp: cfg[index] as i32,
                        code: cfg[index + 1] as i32,
                    });
                }
                let base = (if secondary { 0x7f9578 } else { 0x7f9708 }) + cfg[3] * 0x650;
                assert!(ptr >= base && (ptr - base).is_multiple_of(8));
                Some(records[(ptr - base) as usize / 8 + if secondary { 4 } else { 0 }])
            };
            let mut state = ScheduledEvents {
                active: cfg[0] as u8,
                blocked: cfg[1] as u8,
                primary: State {
                    pending: initial[0] as u8,
                    event: resolve(initial[2], false),
                    flags: 0xdeadbeef,
                    ..Default::default()
                },
                secondary: PendingEvent {
                    pending: initial[1] as u8,
                    event: resolve(initial[3], true),
                },
            };
            let mut primary = Timetable {
                records: records[..4].to_vec(),
                count: initial[8] as i32,
                cursor: initial[6] as i32,
                enabled: initial[4] as u8,
            };
            let mut secondary = Timetable {
                records: records[4..].to_vec(),
                count: initial[9] as i32,
                cursor: initial[7] as i32,
                enabled: initial[5] as u8,
            };
            poll(&mut state, &mut primary, &mut secondary, cfg[2] as i32).unwrap();
            assert_eq!(state.primary.pending, expected[0] as u8, "poll{case}");
            assert_eq!(state.secondary.pending, expected[1] as u8, "poll{case}");
            assert_eq!(
                state.primary.event,
                resolve(expected[2], false),
                "poll{case}"
            );
            assert_eq!(
                state.secondary.event,
                resolve(expected[3], true),
                "poll{case}"
            );
            assert_eq!(state.primary.flags, 0xdeadbeef);
            assert_eq!(
                [
                    u32::from(primary.enabled),
                    u32::from(secondary.enabled),
                    primary.cursor as u32,
                    secondary.cursor as u32,
                    primary.count as u32,
                    secondary.count as u32
                ],
                expected[4..],
                "poll{case}"
            );
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original AI event consumption observations"]
    fn matches_native_event_consumption() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_EVENT")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAIEVT1");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let input: [u32; 12] = std::array::from_fn(|i| word(at + i * 4));
            at += 48;
            let output: [u32; 7] = std::array::from_fn(|i| word(at + i * 4));
            at += 28;
            let expected_calls: Vec<_> =
                (0..output[4] as usize).map(|i| word(at + i * 32)).collect();
            at += output[4] as usize * 32;
            let record = Record {
                timestamp: input[2] as i32,
                code: input[3] as i32,
            };
            let mut state = State {
                pending: input[0] as u8,
                event: (input[1] != 0).then_some(record),
                flags: 0xa55a0000 | input[10] | input[11] << 8,
                disabled: input[5] as u8,
                mode: input[6] as i32,
                category: input[7] as i32,
            };
            let mut categories = vec![0; (state.category.max(0) + 1) as usize];
            if state.category >= 0 {
                categories[state.category as usize] = input[8] as u8;
            }
            let calls = std::cell::RefCell::new(Vec::new());
            let replacement = Record {
                timestamp: 123,
                code: 456,
            };
            let result = consume(
                &mut state,
                input[4] as i32,
                &categories,
                |code, _| {
                    assert_eq!(code, record.code);
                    calls.borrow_mut().extend([0x50e94f, 0x4e6960]);
                    Ok(())
                },
                |_| {
                    calls.borrow_mut().push(0x478130);
                    Ok((input[9] != 0).then_some(replacement))
                },
            );
            assert_eq!(result.is_err(), output[5] != 0, "event{case}");
            assert_eq!(state.pending, output[0] as u8, "event{case}");
            assert_eq!(
                state.flags,
                0xa55a0000 | output[2] | output[3] << 8,
                "event{case}"
            );
            let actual_ptr = state.event.map_or(0, |r| {
                if r == replacement {
                    input[9]
                } else {
                    0x10011000
                }
            });
            assert_eq!(actual_ptr, output[1], "event{case}");
            assert_eq!(*calls.borrow(), expected_calls, "event{case}");
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original primary AI queue observations"]
    fn matches_native_primary_queue() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_PRIMARY_QUEUE")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAIQ478");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for _ in 0..word(12) {
            let length = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&bytes[at..at + length]).unwrap();
            at += length;
            let op = word(at);
            let input: [i32; 6] = std::array::from_fn(|i| word(at + 4 + i * 4) as i32);
            at += 28;
            let count = word(at) as usize;
            at += 4;
            let records: Vec<_> = (0..count)
                .map(|i| Record {
                    timestamp: word(at + i * 8) as i32,
                    code: word(at + i * 8 + 4) as i32,
                })
                .collect();
            at += count * 8;
            let expected = records[input[5] as usize];
            let mut queue = Timetable {
                records,
                count: input[2],
                cursor: input[3],
                enabled: input[4] as u8,
            };
            let actual = if op == 0 {
                queue.select(input[1])
            } else {
                queue.advance()
            }
            .unwrap();
            assert_eq!(actual, expected, "{name}");
            assert_eq!(queue.cursor, word(at + 4) as i32, "{name}");
            assert_eq!(queue.enabled, bytes[at + 8], "{name}");
            assert_eq!(
                word(at),
                0x7f9708_u32
                    .wrapping_add(input[0] as u32 * 0x650)
                    .wrapping_add(input[5] as u32 * 8),
                "{name}"
            );
            at += 14;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires native Lua timetable setter observations"]
    fn matches_native_timetable_configuration() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_TIMETABLE")).unwrap();
        assert_eq!(&bytes[..8], b"MTAITBL1");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let mut at = 12;
        for case in 0..count {
            let reset = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) != 0;
            at += 4;
            let arguments = std::array::from_fn(|i| {
                f64::from_le_bytes(bytes[at + i * 8..at + i * 8 + 8].try_into().unwrap())
            });
            at += 32;
            let mut profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
            at += 0x650;
            configure_timetable(&mut profile, reset, arguments).unwrap();
            let differences: Vec<_> = profile
                .iter()
                .zip(&bytes[at..at + 0x650])
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .take(16)
                .collect();
            assert!(differences.is_empty(), "timetable{case}: {differences:?}");
            at += 0x650;
        }
        assert_eq!(at, bytes.len());
    }
}
