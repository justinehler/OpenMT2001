//! Offline participant controls recovered from49ef20/49ebc0/4a1b40.
//! Slot indices, token identities and per-player difficulty are distinct.
pub const SLOTS: usize = 6;
pub const TOKENS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Empty,
    Human,
    Ai,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
    pub kind: Kind,
    pub token: Option<usize>,
    pub name: Vec<u8>,
    pub ready: bool,
    /// Record+44 is retained independently of the selected game difficulty.
    pub difficulty: i32,
}
impl Participant {
    fn empty(difficulty: i32) -> Self {
        Self {
            kind: Kind::Empty,
            token: None,
            name: vec![],
            ready: false,
            difficulty,
        }
    }
}

pub struct Roster {
    slots: [Participant; SLOTS],
    token_names: [Vec<u8>; TOKENS],
}

fn validate_name(name: &[u8]) -> Result<(), String> {
    if name.len() >= 32 || name.contains(&0) {
        return Err("participant name exceeds the original32-byte field".into());
    }
    Ok(())
}

impl Roster {
    ///49f3a0 makes the initial local human ready. Five other slots remain
    /// empty until explicit user action. The caller supplies retained+44 words;
    /// they are never replaced with a guessed global/AI difficulty.
    pub fn new(
        name: &[u8],
        token: usize,
        token_names: [Vec<u8>; TOKENS],
        retained_difficulties: [i32; SLOTS],
    ) -> Result<Self, String> {
        validate_name(name)?;
        if name.is_empty() || token >= TOKENS {
            return Err("invalid local participant".into());
        }
        for name in &token_names {
            validate_name(name)?;
        }
        let mut slots = retained_difficulties.map(Participant::empty);
        slots[0].kind = Kind::Human;
        slots[0].token = Some(token);
        slots[0].name = name.to_vec();
        slots[0].ready = true;
        Ok(Self { slots, token_names })
    }

    pub fn slots(&self) -> &[Participant; SLOTS] {
        &self.slots
    }

    /// Offline state2 -> AI1 -> empty2; local slot0 cannot be replaced.
    pub fn toggle_ai(&mut self, slot: usize) -> Result<(), String> {
        if slot == 0 || slot >= SLOTS {
            return Err("only nonlocal participant slots can change type".into());
        }
        if self.slots[slot].kind == Kind::Ai {
            let difficulty = self.slots[slot].difficulty;
            self.slots[slot] = Participant::empty(difficulty);
        } else if self.slots[slot].kind == Kind::Empty {
            let token = (0..TOKENS)
                .find(|t| !self.slots.iter().any(|p| p.token == Some(*t)))
                .ok_or("no unused participant token")?;
            self.slots[slot].kind = Kind::Ai;
            self.slots[slot].token = Some(token);
            self.slots[slot].name = self.token_names[token].clone();
            self.slots[slot].ready = true;
        } else {
            return Err("offline slot is not an AI or empty slot".into());
        }
        Ok(())
    }

    ///49ebc0 scans every token field, not just active rows. A current row's
    /// token is also excluded, so cycling always chooses another unused token.
    pub fn cycle_token(&mut self, slot: usize, direction: i32) -> Result<(), String> {
        if !matches!(direction, -1 | 1) {
            return Err("participant token direction must be-1 or1".into());
        }
        let participant = self
            .slots
            .get(slot)
            .ok_or("participant slot outside six rows")?;
        if participant.kind == Kind::Empty {
            return Err("empty participant has no token control".into());
        }
        let current = participant.token.ok_or("active participant has no token")?;
        let mut token = current as i32 + TOKENS as i32;
        for _ in 0..TOKENS {
            token = (token + direction) % TOKENS as i32;
            if !self.slots.iter().any(|p| p.token == Some(token as usize)) {
                self.slots[slot].token = Some(token as usize);
                if self.slots[slot].kind == Kind::Ai {
                    self.slots[slot].name = self.token_names[token as usize].clone();
                }
                return Ok(());
            }
        }
        Err("no unused participant token".into())
    }

    pub fn rename(&mut self, slot: usize, name: &[u8]) -> Result<(), String> {
        validate_name(name)?;
        let participant = self
            .slots
            .get_mut(slot)
            .ok_or("participant slot outside six rows")?;
        if participant.kind == Kind::Empty {
            return Err("empty participant has no name input".into());
        }
        participant.name = name.to_vec();
        Ok(())
    }

    pub fn toggle_ready(&mut self) {
        self.slots[0].ready = !self.slots[0].ready;
    }

    ///49ead0: an unready human blocks Start; an AI or ready human counts.
    /// The original count includes the ready human, so one AI is enough.
    pub fn start_enabled(&self) -> bool {
        !self.slots.iter().any(|p| p.kind == Kind::Human && !p.ready)
            && self
                .slots
                .iter()
                .filter(|p| p.kind == Kind::Ai || p.kind == Kind::Human && p.ready)
                .count()
                > 1
    }

    ///4a2141's final consumer checks readiness/name/token separately from the
    /// button's eligible-count policy. UI callers must also honor start_enabled.
    /// Full records move in slot order; moved-from rows are cleared, including
    /// their old+44 difficulty word. No token-sorted or fixed-AI roster is made.
    pub fn compact_ready(&mut self) -> Result<Vec<Participant>, String> {
        let mut tokens = [false; TOKENS];
        for participant in self.slots.iter().filter(|p| p.kind != Kind::Empty) {
            let token = participant
                .token
                .filter(|&t| t < TOKENS)
                .ok_or("active participant token invalid")?;
            if tokens[token] {
                return Err("participant tokens are not unique".into());
            }
            tokens[token] = true;
            if participant.name.is_empty() || !participant.ready {
                return Err("participant name or readiness incomplete".into());
            }
        }
        let mut next = 0;
        for index in 0..SLOTS {
            if self.slots[index].kind == Kind::Empty {
                continue;
            }
            if next != index {
                self.slots[next] = self.slots[index].clone();
                self.slots[index] = Participant::empty(0);
            }
            next += 1;
        }
        Ok(self.slots[..next].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roster() -> Roster {
        Roster::new(
            b"Justin2",
            4,
            std::array::from_fn(|i| format!("Token{i}").into_bytes()),
            [0, 1, 2, 3, 4, 5],
        )
        .unwrap()
    }
    #[test]
    fn human_plus_one_explicit_ai_enables_start_and_ready_blocks_it() {
        let mut r = roster();
        assert_eq!(r.slots.iter().filter(|p| p.kind == Kind::Human).count(), 1);
        assert_eq!(r.slots.iter().filter(|p| p.kind == Kind::Empty).count(), 5);
        assert!(!r.start_enabled());
        assert!(r.toggle_ai(0).is_err());
        r.toggle_ai(3).unwrap();
        assert_eq!(r.slots[3].token, Some(0));
        assert_eq!(r.slots[3].name, b"Token0");
        assert_eq!(r.slots[3].difficulty, 3);
        assert!(r.start_enabled());
        r.toggle_ready();
        assert!(!r.start_enabled());
        r.toggle_ready();
        assert!(r.start_enabled());
        r.toggle_ai(3).unwrap();
        assert!(!r.start_enabled());
        assert_eq!(r.slots[3], Participant::empty(3));
    }
    #[test]
    fn token_cycles_skip_all_other_choices_and_keep_human_name() {
        let mut r = roster();
        for i in 1..SLOTS {
            r.toggle_ai(i).unwrap();
        }
        for direction in [-1, 1] {
            for slot in 0..SLOTS {
                let old = r.slots[slot].token;
                r.cycle_token(slot, direction).unwrap();
                assert_ne!(old, r.slots[slot].token);
                let mut tokens = r.slots.iter().map(|p| p.token.unwrap()).collect::<Vec<_>>();
                tokens.sort();
                tokens.dedup();
                assert_eq!(tokens.len(), SLOTS);
            }
        }
        assert_eq!(r.slots[0].name, b"Justin2");
        for slot in 1..SLOTS {
            assert_eq!(
                r.slots[slot].name,
                r.token_names[r.slots[slot].token.unwrap()]
            );
        }
    }
    #[test]
    fn final_name_guard_and_compaction_preserve_record_order_and_difficulty() {
        let mut r = roster();
        r.toggle_ai(5).unwrap();
        r.toggle_ai(2).unwrap();
        r.rename(2, b"").unwrap();
        assert!(
            r.start_enabled(),
            "visible count policy is separate from final name guard"
        );
        let before = r.slots.clone();
        assert!(r.compact_ready().is_err());
        assert_eq!(before, r.slots);
        r.rename(2, b"Opponent").unwrap();
        let participants = r.compact_ready().unwrap();
        assert_eq!(
            participants
                .iter()
                .map(|p| p.difficulty)
                .collect::<Vec<_>>(),
            [0, 2, 5]
        );
        assert_eq!(participants[1].name, b"Opponent");
        assert_eq!(r.slots[5], Participant::empty(0));
        assert_eq!(
            r.slots[4],
            Participant::empty(4),
            "untouched empty rows retain prior difficulty"
        );
        assert!(r.rename(6, b"x").is_err());
        assert!(r.rename(0, &[b'x'; 32]).is_err());
        assert!(r.rename(0, b"x\0y").is_err());
    }
}
