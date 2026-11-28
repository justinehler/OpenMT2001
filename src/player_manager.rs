//! Original48c5d0 manager linkage. Constructors are explicit dependencies.
//! References are opaque u32 IDs; they are never dereferenced as host addresses.
pub struct Player {
    pub reference: u32,
    pub bytes: Vec<u8>,
    pub auxiliaries: Vec<Auxiliary>,
}

pub struct Auxiliary {
    pub reference: u32,
    pub bytes: [u8; 0x30],
}

/// Native4895b0 boundaries. RemovalRequest is the stack command of type9
/// sent to450490; the host must execute it before later network gates are read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CashEvent {
    RemovalRequest(usize),
    Command([i32; 8]),
    RemovePlayer(usize),
}

pub struct CashControls {
    pub network_enabled: bool,
    pub network_override: bool,
}

/// Original global RNG seeding stream and registry head/tail. References are
/// logical byte addresses in the shared arena, never native host pointers.
pub struct RandomRegistry {
    pub process: [u32; 3],
    pub head: u32,
    pub tail: u32,
}

impl RandomRegistry {
    /// Original4e5580. Update each sole-owned node's state and return its next
    /// reference through `write_state`. Coefficients and links stay untouched.
    /// `last_seed` is the live940834 global, written only after traversal.
    pub fn reseed(
        &mut self,
        seed: u32,
        last_seed: &mut u32,
        write_state: impl FnMut(u32, u32) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.reseed_with_process_node(seed, last_seed, None, write_state)
    }

    /// Include the process stream's own registered node (original93c818).
    /// `process` remains authoritative for that node's first three words;
    /// the callback supplies its linkage, as for every other registered node.
    pub fn reseed_with_process_node(
        &mut self,
        seed: u32,
        last_seed: &mut u32,
        process_reference: Option<u32>,
        mut write_state: impl FnMut(u32, u32) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.process[0] = seed;
        let mut node = self.head;
        while node != 0 {
            self.process[0] = self.process[0]
                .wrapping_mul(self.process[1])
                .wrapping_add(self.process[2]);
            let state = (self.process[0] >> 16) & 0x7fff;
            if Some(node) == process_reference {
                //4e55b6 aliases the just-written4e55a9 process state. Later
                // nodes draw from this truncated value, not the full LCG word.
                self.process[0] = state;
            }
            node = write_state(node, state)?;
        }
        *last_seed = seed;
        Ok(())
    }

    /// Original4e55d0: register and seed an embedded twenty-byte RNG record.
    pub fn initialize_rng(
        &mut self,
        reference: u32,
        bytes: &mut [u8],
        link_tail: &mut impl FnMut(u32, u32) -> Result<(), String>,
    ) -> Result<(), String> {
        if reference == 0 || bytes.len() < 20 {
            return Err("invalid RNG storage".into());
        }
        put(bytes, 4, 0x343fd);
        put(bytes, 8, 0x269ec3);
        put(bytes, 0, 0);
        if self.head == 0 {
            self.head = reference;
            self.tail = reference;
            put(bytes, 12, 0);
            put(bytes, 16, 0);
        } else {
            link_tail(self.tail, reference)?;
            put(bytes, 16, self.tail);
            self.tail = reference;
            put(bytes, 12, 0);
        }
        self.process[0] = self.process[0]
            .wrapping_mul(self.process[1])
            .wrapping_add(self.process[2]);
        put(bytes, 0, (self.process[0] >> 16) & 0x7fff);
        Ok(())
    }

    /// Original41bae0 with nested4e55d0. The arena updates an existing RNG
    /// node's forward link through `link_tail`; no duplicate node state is kept.
    pub fn initialize_account(
        &mut self,
        account: &mut Auxiliary,
        link_tail: &mut impl FnMut(u32, u32) -> Result<(), String>,
    ) -> Result<(), String> {
        let node = account
            .reference
            .checked_add(0x1c)
            .ok_or("RNG reference overflow")?;
        if account.reference == 0 {
            return Err("null account reference".into());
        }
        self.initialize_rng(node, &mut account.bytes[0x1c..], link_tail)?;
        let seed = word(&account.bytes, 0x1c);
        let state = seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        put(&mut account.bytes, 0x1c, state);
        put(&mut account.bytes, 0x18, 0);
        account.bytes[8] = 0;
        put(&mut account.bytes, 0, 5 + ((state >> 16) & 0x7fff) % 6);
        Ok(())
    }
}

impl Player {
    /// Original0041cd70 on this player's +21c auxiliary. Keep the countdown in
    /// its constructor-owned record. A returned48cab0 command must be dispatched
    /// by the caller; command routing/network effects are not simulated here.
    pub fn advance_hourly_account(
        &mut self,
        player_count: i32,
    ) -> Result<Option<[i32; 8]>, String> {
        if self.bytes.len() < 0x220 {
            return Err("truncated hourly player".into());
        }
        let reference = word(&self.bytes, 0x21c);
        let account = self
            .auxiliaries
            .iter_mut()
            .find(|a| a.reference == reference)
            .ok_or("hourly player account reference is unresolved")?;
        let old = word(&account.bytes, 0);
        put(&mut account.bytes, 0, old.wrapping_sub(1));
        if old != 0 || word(&account.bytes, 0x18) != 0 || player_count <= 1 {
            return Ok(None);
        }
        if word(&account.bytes, 0x14) != self.reference {
            return Err("hourly account owner reference does not match player".into());
        }
        Ok(Some([
            -1,
            i32::from(self.bytes[0x1bc]),
            0x31,
            0,
            0,
            0,
            0,
            -999,
        ]))
    }

    /// Original47d5f0: two RNG draws and the current build-time row. The caller
    /// stores only the low word; the clock is original822d40, supplied by host.
    pub fn initial_count(&mut self, profiles: &[[u8; 0x650]], tick: u32) -> Result<u32, String> {
        if self.bytes.len() < 0x13f8 {
            return Err("truncated AI player".into());
        }
        let character = word(&self.bytes, 0x150) as usize;
        let profile = profiles
            .get(character)
            .filter(|_| character < 10)
            .ok_or("AI character leaves profile table")?;
        let offset = 0x614_i64 + i64::from(word(&self.bytes, 0x13f4) as i32) * 20;
        let offset = usize::try_from(offset).map_err(|_| "AI build-time row underflow")?;
        let row = profile
            .get(offset..offset.saturating_add(4))
            .ok_or("AI build-time row overflow")?;
        let base = u32::from_le_bytes(row.try_into().unwrap());
        let multiplier = word(&self.bytes, 0x228);
        let increment = word(&self.bytes, 0x22c);
        let first = word(&self.bytes, 0x224)
            .wrapping_mul(multiplier)
            .wrapping_add(increment);
        let remainder = ((first >> 16) & 0x7fff) % 11;
        let second = first.wrapping_mul(multiplier).wrapping_add(increment);
        put(&mut self.bytes, 0x224, second);
        let delta = tick.wrapping_add(25 * remainder);
        Ok(if (second >> 16) & 1 == 1 {
            base.wrapping_add(delta)
        } else {
            base.wrapping_sub(delta)
        })
    }

    /// Original48a1c0 direct common constructor. The first dependency must
    /// perform41bae0, including its RNG registration; the second supplies raw
    /// allocation. Both use opaque references, never host pointers.
    pub fn initialize_common(
        &mut self,
        default_name: &[u8],
        make_account: &mut impl FnMut() -> Result<Auxiliary, String>,
        make_command: &mut impl FnMut() -> Result<Option<Auxiliary>, String>,
    ) -> Result<(), String> {
        if self.reference == 0 || self.bytes.len() < 0x224 {
            return Err("common player constructor requires valid storage".into());
        }
        let name = &default_name[..default_name
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(default_name.len())];
        if name.len() >= 32 {
            return Err("default player name exceeds field".into());
        }
        put(&mut self.bytes, 0, 0x525dc8);
        put(&mut self.bytes, 4, 0);
        put(&mut self.bytes, 0x1c0, (-100_f32).to_bits());
        self.bytes[0x158..0x158 + name.len()].copy_from_slice(name);
        self.bytes[0x158 + name.len()] = 0;
        self.bytes[0x178..0x1ac].fill(0);
        put(&mut self.bytes, 8, 10);
        for at in [0x214, 0x218] {
            put(&mut self.bytes, at, 0);
        }
        self.bytes[0x10] = 1;
        self.bytes[0x14..0x144].fill(0);
        self.bytes[0x144] = 1;
        self.bytes[0x145] = 0;
        self.bytes[0x11..0x13].fill(0);
        self.bytes[0x1c4..0x20c].fill(0);
        for at in [0x148, 0x1ac, 0x1b0, 0x1b4, 0x1b8, 0x14c] {
            put(&mut self.bytes, at, 0);
        }
        let mut account = make_account()?;
        if account.reference == 0 {
            return Err("common account constructor returned null".into());
        }
        put(&mut self.bytes, 0x21c, account.reference);
        put(&mut account.bytes, 0x14, self.reference);
        self.auxiliaries.push(account);
        if let Some(mut command) = make_command()? {
            if command.reference == 0 {
                return Err("command allocation returned invalid reference".into());
            }
            put(&mut command.bytes, 4, 0x21);
            put(&mut command.bytes, 0xc, 0);
            command.bytes[0x10] = 0;
            put(&mut command.bytes, 0, 0x525dc0);
            put(&mut self.bytes, 0x210, command.reference);
            self.auxiliaries.push(command);
        } else {
            put(&mut self.bytes, 0x210, 0);
        }
        Ok(())
    }
}

pub struct Manager {
    /// Availability bytes, counters, and six opaque player references.
    pub bytes: [u8; 0x28],
    pub players: Vec<Player>,
}

pub struct Creation<'a> {
    pub kind: i32,
    pub character: i32,
    pub name: &'a [u8],
    pub initial_cash: f32,
}

pub(crate) fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
pub(crate) fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

impl Manager {
    /// Original4895b0 midnight cash-status pass. Status bytes are the first
    /// byte of each native eight-byte row, supplied by the caller unchanged.
    /// Callback effects are synchronous and may alter later players or gates.
    pub fn update_cash_status(
        &mut self,
        hour: u32,
        statuses: &mut [u8; 6],
        controls: &mut CashControls,
        mut callback: impl FnMut(
            CashEvent,
            &mut Self,
            &mut [u8; 6],
            &mut CashControls,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        if hour != 24 {
            return Ok(());
        }
        let command =
            |index: usize, kind| CashEvent::Command([index as i32, -1, kind, 0, 0, 0, 0, -999]);
        let mut index = 0;
        while index < self.players.len() {
            let player = &self.players[index];
            if player.bytes.len() < 0x14 || index >= statuses.len() {
                return Err("midnight player leaves original cash-status table".into());
            }
            if player.bytes[0x10] != 0 {
                let cash = f32::from_bits(word(&player.bytes, 4));
                // FCOMP/FNSTSW tests C0 only: unordered cash follows the
                // negative path too, while negative zero does not.
                if cash < 0. || cash.is_nan() {
                    if statuses[index] != 0 {
                        if !controls.network_enabled || controls.network_override {
                            callback(CashEvent::RemovalRequest(index), self, statuses, controls)?;
                        }
                        if !controls.network_enabled || controls.network_override {
                            callback(command(index, 0x2d), self, statuses, controls)?;
                            callback(CashEvent::RemovePlayer(index), self, statuses, controls)?;
                        }
                    } else {
                        statuses[index] = 1;
                        if !controls.network_enabled || controls.network_override {
                            callback(command(index, 0x2b), self, statuses, controls)?;
                        }
                    }
                } else {
                    if statuses[index] != 0 {
                        // Recovery notification is deliberately not network-gated.
                        callback(command(index, 0x2c), self, statuses, controls)?;
                    }
                    statuses[index] = 0;
                }
            }
            index += 1;
        }
        Ok(())
    }

    /// Original48c5a0: initialize six free slots, preserving padding bytes6/7.
    pub fn initialize(mut bytes: [u8; 0x28]) -> Self {
        bytes[..6].fill(1);
        bytes[8..].fill(0);
        Self {
            bytes,
            players: vec![],
        }
    }

    /// Copy the live accounts into the player records (cash, ledgers, owned
    /// categories, retail and housing counters). The original keeps a single
    /// record per player and writes sales straight to it (0x4625c0 adds each
    /// unit to +0x1b4), so the records must match the accounts wherever
    /// original code would read them. The roster lengths must agree; player
    /// removal reconciles both representations.
    pub fn publish_accounts(
        &mut self,
        accounts: &[crate::economy::PlayerAccount],
    ) -> Result<(), String> {
        if self.players.len() != accounts.len() {
            return Err("runtime manager/account roster length mismatch".into());
        }
        for (player, account) in self.players.iter_mut().zip(accounts) {
            account.write_runtime(&mut player.bytes)?;
        }
        Ok(())
    }

    /// The 440-byte player prefixes a V2 save stores, taken after the live
    /// accounts are published.
    pub fn account_prefixes(
        &mut self,
        accounts: &[crate::economy::PlayerAccount],
    ) -> Result<Vec<[u8; 440]>, String> {
        self.publish_accounts(accounts)?;
        self.players
            .iter()
            .map(|player| {
                player
                    .bytes
                    .get(4..444)
                    .ok_or_else(|| "truncated runtime player prefix".to_string())?
                    .try_into()
                    .map_err(|_| "invalid runtime player prefix".into())
            })
            .collect()
    }

    /// Resolve a registered RNG node into its sole owning player allocation.
    /// Returns false for a node owned by another world subsystem.
    pub fn link_rng_tail(&mut self, reference: u32, next: u32) -> bool {
        for player in &mut self.players {
            if player.bytes.len() >= 0x238 && player.reference.checked_add(0x224) == Some(reference)
            {
                put(&mut player.bytes, 0x230, next);
                return true;
            }
            for auxiliary in &mut player.auxiliaries {
                if auxiliary.reference.checked_add(0x1c) == Some(reference) {
                    put(&mut auxiliary.bytes, 0x28, next);
                    return true;
                }
            }
        }
        false
    }

    /// Original4d7ff0..4d80a5. Global postludes are outside this loop.
    /// Event callbacks may update profile links, so resolve the player after
    /// all four calls. Re-read the next profile's active byte each iteration.
    pub fn initialize_ai(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        difficulty: i32,
        refresh_count: bool,
        event: &mut impl FnMut(&mut Self, &mut [[u8; 0x650]], usize, usize) -> Result<(), String>,
        count: &mut impl FnMut(&mut Player, &[[u8; 0x650]]) -> Result<u16, String>,
    ) -> Result<(), String> {
        if profiles.len() < 10 {
            return Err("AI initialization requires ten profiles".into());
        }
        for index in 0..10 {
            if profiles[index][4] == 0 {
                continue;
            }
            for event_index in 0..4 {
                event(self, profiles, index, event_index)?;
            }
            let reference = word(&profiles[index], 0);
            let player = self
                .players
                .iter_mut()
                .find(|player| player.reference == reference)
                .ok_or("active AI profile has no linked player")?;
            if player.bytes.len() < 0x13f8 {
                return Err("active AI player prefix truncated".into());
            }
            put(
                &mut player.bytes,
                0x13f4,
                match difficulty {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                },
            );
            if refresh_count {
                let result = count(player, profiles)?;
                player.bytes[0xb4c..0xb4e].copy_from_slice(&result.to_le_bytes());
            }
        }
        Ok(())
    }

    /// Initial image value only. Level scripts mutate541c78 before registration;
    /// the host must pass that live value through Creation::initial_cash.
    pub fn initial_cash(image: &crate::pe::PeImage<'_>) -> Result<f32, String> {
        Ok(f32::from_le_bytes(
            image.at(0x541c78, 4)?.try_into().unwrap(),
        ))
    }

    /// `construct` must run the appropriate common/full constructor, returning
    /// initialized storage and a unique nonzero reference. This method never
    /// substitutes zeroed bytes for those constructor effects.
    pub fn register(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        request: Creation<'_>,
        construct: &mut impl FnMut(&mut Self, i32) -> Result<Player, String>,
    ) -> Result<bool, String> {
        let Creation {
            kind,
            character,
            name,
            initial_cash,
        } = request;
        if kind == -1 {
            return Ok(false);
        }
        let Some(slot) = self.bytes[..6].iter().position(|available| *available != 0) else {
            return Ok(false);
        };
        let profile = if kind == 1 || kind == 2 {
            Some(
                profiles
                    .get_mut(usize::try_from(character).map_err(|_| "negative AI profile")?)
                    .ok_or("AI profile leaves table")?,
            )
        } else {
            None
        };
        if (0..=2).contains(&kind) {
            let mut player = construct(self, kind)?;
            let required = if kind == 0 { 0x224 } else { 0x1e50 };
            if player.reference == 0
                || player.bytes.len() < required
                || self.players.iter().any(|p| p.reference == player.reference)
            {
                return Err("constructor returned invalid player storage/reference".into());
            }
            if kind == 0 {
                // Native class marker is retained as data, never used for calls.
                put(&mut player.bytes, 0, 0x525e04);
                put(&mut player.bytes, 0x220, 0);
            }
            put(&mut self.bytes, 0x10 + slot * 4, player.reference);
            if let Some(profile) = profile {
                put(profile, 0, player.reference);
                profile[4] = 1;
            }
            self.players.push(player);
        }
        // Unsupported native kinds fall through to the existing slot reference.
        // Native null/invalid references fault; report this instead of host UB.
        let reference = word(&self.bytes, 0x10 + slot * 4);
        let player = self
            .players
            .iter_mut()
            .find(|p| p.reference == reference)
            .ok_or("player manager references absent storage")?;
        if player.bytes.len() < 0x1bd {
            return Err("player prefix is truncated".into());
        }
        player.bytes[0x1bc] = slot as u8;
        put(&mut player.bytes, 0x154, kind as u32);
        let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())];
        let destination = player
            .bytes
            .get_mut(0x158..0x158 + name.len() + 1)
            .ok_or("player name leaves storage")?;
        destination[..name.len()].copy_from_slice(name);
        destination[name.len()] = 0;
        put(&mut player.bytes, 4, initial_cash.to_bits());
        put(&mut player.bytes, 0x150, character as u32);
        self.bytes[slot] = 0;
        let count = word(&self.bytes, 8).wrapping_add(1);
        put(&mut self.bytes, 8, count);
        if kind != 2 {
            let count = word(&self.bytes, 0xc).wrapping_add(1);
            put(&mut self.bytes, 0xc, count);
        }
        if word(&player.bytes, 0x154) == 2 {
            player.bytes[0x10] = 0;
        }
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original midnight cash-status observations"]
    fn matches_native_cash_status() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_HOURLY_CASH_STATUS")).unwrap();
        assert_eq!(&data[..8], b"MTHCASH2");
        assert_eq!(word(&data, 8), 2);
        let size = word(&data, 16) as usize;
        assert_eq!(word(&data, 24), 48);
        let mut at = 28;
        for case in 0..word(&data, 12) {
            let hour = word(&data, at);
            let mut controls = CashControls {
                network_enabled: word(&data, at + 4) != 0,
                network_override: word(&data, at + 8) != 0,
            };
            let count = word(&data, at + 12) as usize;
            let calls = word(&data, at + 16);
            let mutations: [u32; 3] = std::array::from_fn(|i| word(&data, at + 20 + i * 4));
            at += 32;
            let mut manager = Manager::initialize([0; 0x28]);
            let mut expected_players = vec![];
            for index in 0..count {
                manager.players.push(Player {
                    reference: 0x10020000 + index as u32 * 0x400,
                    bytes: data[at..at + size].to_vec(),
                    auxiliaries: vec![],
                });
                at += size;
                expected_players.push(data[at..at + size].to_vec());
                at += size;
            }
            let mut status_rows = data[at..at + 48].to_vec();
            let mut statuses = std::array::from_fn(|i| status_rows[i * 8]);
            at += 48;
            let expected_status = &data[at..at + 48];
            at += 48;
            let mut expected_calls = vec![];
            for _ in 0..calls {
                let address = word(&data, at);
                let kind = word(&data, at + 4);
                let event = match kind {
                    1 => {
                        assert_eq!(address, 0x48cab0);
                        assert_eq!(word(&data, at + 12), 8);
                        CashEvent::Command(std::array::from_fn(|i| {
                            word(&data, at + 16 + i * 4) as i32
                        }))
                    }
                    2 => {
                        assert_eq!(address, 0x450490);
                        let record = &data[at + 52..at + 84];
                        assert_eq!(word(record, 0), 0x525db8);
                        assert_eq!(word(record, 4), 9);
                        assert_eq!(word(record, 12), 0);
                        assert_eq!(record[16], 1);
                        assert_eq!(word(record, 24), 0);
                        CashEvent::RemovalRequest(word(record, 20) as usize)
                    }
                    3 => {
                        assert_eq!(address, 0x48cc50);
                        CashEvent::RemovePlayer(word(&data, at + 16) as usize)
                    }
                    _ => panic!("unknown native cash callback"),
                };
                expected_calls.push(event);
                at += 84;
            }
            let mut actual_calls = vec![];
            manager
                .update_cash_status(
                    hour,
                    &mut statuses,
                    &mut controls,
                    |event, _, statuses, controls| {
                        actual_calls.push(event);
                        match event {
                            CashEvent::RemovalRequest(_) => {
                                if mutations[0] != u32::MAX {
                                    controls.network_enabled = mutations[0] != 0;
                                }
                                if mutations[1] != u32::MAX {
                                    controls.network_override = mutations[1] != 0;
                                }
                            }
                            CashEvent::Command(args) if mutations[2] != u32::MAX => {
                                statuses[args[0] as usize] = mutations[2] as u8;
                            }
                            _ => {}
                        }
                        Ok(())
                    },
                )
                .unwrap();
            for (i, status) in statuses.into_iter().enumerate() {
                status_rows[i * 8] = status;
            }
            assert_eq!(status_rows, expected_status, "case {case} full status rows");
            assert_eq!(actual_calls, expected_calls, "case {case} callback order");
            for (player, expected) in manager.players.iter().zip(expected_players) {
                assert_eq!(player.bytes, expected, "case {case} full player");
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original hourly player-account observations"]
    fn matches_native_hourly_account() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PLAYER_CONTROLLER")).unwrap();
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(&data[..8], b"MTPCTRL1");
        let mut at = 12;
        for case in 0..word(8) {
            let count = word(at) as i32;
            let slot = word(at + 4) as u8;
            at += 8;
            let mut player = super::Player {
                reference: 0x10011000,
                bytes: vec![0; 0x224],
                auxiliaries: vec![super::Auxiliary {
                    reference: 0x10010000,
                    bytes: data[at..at + 48].try_into().unwrap(),
                }],
            };
            player.bytes[0x21c..0x220].copy_from_slice(&0x10010000_u32.to_le_bytes());
            player.bytes[0x1bc] = slot;
            at += 48;
            let command = player.advance_hourly_account(count).unwrap();
            assert_eq!(
                player.auxiliaries[0].bytes.as_slice(),
                &data[at..at + 48],
                "case {case} full account"
            );
            at += 48;
            let calls = word(at);
            at += 4;
            let expected = if calls == 0 {
                None
            } else {
                assert_eq!(calls, 1);
                let args = std::array::from_fn(|i| word(at + i * 4) as i32);
                at += 32;
                Some(args)
            };
            assert_eq!(command, expected, "case {case} command");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original4e5580 observations"]
    fn matches_native_registry_reseed() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RNG_RESEED")).unwrap();
        assert_eq!(&bytes[..8], b"MTRNGS01");
        assert_eq!(word(&bytes, 8), 1);
        let mut at = 16;
        for case in 0..word(&bytes, 12) {
            let fields: Vec<_> = (0..16).map(|i| word(&bytes, at + i * 4)).collect();
            at += 64;
            assert_eq!(fields[14], 0x100f0000);
            assert_eq!(fields[15], 0);
            let mut registry = RandomRegistry {
                process: fields[2..5].try_into().unwrap(),
                head: fields[6],
                tail: fields[7],
            };
            let mut last_seed = fields[5];
            let mut nodes = Vec::new();
            for _ in 0..fields[1] {
                let reference = word(&bytes, at);
                let before = bytes[at + 4..at + 24].to_vec();
                let after = bytes[at + 24..at + 44].to_vec();
                nodes.push((reference, before, after));
                at += 44;
            }
            registry
                .reseed_with_process_node(
                    fields[0],
                    &mut last_seed,
                    Some(0x93c818),
                    |reference, state| {
                        let node = nodes
                            .iter_mut()
                            .find(|n| n.0 == reference)
                            .ok_or("missing node")?;
                        put(&mut node.1, 0, state);
                        Ok(word(&node.1, 12))
                    },
                )
                .unwrap();
            assert_eq!(registry.process, fields[8..11], "case {case}");
            assert_eq!([last_seed, registry.head, registry.tail], fields[11..14]);
            for (reference, mut actual, expected) in nodes {
                if reference == 0x93c818 {
                    for (i, value) in registry.process.iter().enumerate() {
                        put(&mut actual, i * 4, *value);
                    }
                }
                assert_eq!(actual, expected, "case {case}, node {reference:x}");
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original47d5f0 observations"]
    fn matches_native_initial_count() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_INITIAL_COUNT")).unwrap();
        assert_eq!(&bytes[..8], b"MTAIIC47");
        assert_eq!(word(&bytes, 8), 1);
        let mut at = 20;
        for case in 0..word(&bytes, 12) {
            let fields = [0x13f4, 0x150, 0x224, 0x228, 0x22c];
            let mut player = Player {
                reference: 1,
                bytes: vec![0x5d; word(&bytes, 16) as usize],
                auxiliaries: vec![],
            };
            for field in fields {
                put(&mut player.bytes, field, word(&bytes, at));
                at += 4;
            }
            let table = word(&bytes, at);
            let tick = word(&bytes, at + 4);
            let expected = word(&bytes, at + 8);
            at += 12;
            let mut profiles = vec![[0; 0x650]; 10];
            let index = word(&player.bytes, 0x150) as usize;
            let offset = (0x614_i32 + (word(&player.bytes, 0x13f4) as i32) * 20) as usize;
            put(&mut profiles[index], offset, table);
            let before = player.bytes.clone();
            assert_eq!(
                player.initial_count(&profiles, tick).unwrap(),
                expected,
                "case {case}"
            );
            for field in fields {
                assert_eq!(word(&before, field), word(&bytes, at));
                assert_eq!(word(&player.bytes, field), word(&bytes, at + 4));
                at += 8;
            }
            let mut expected_player = before;
            put(&mut expected_player, 0x224, word(&player.bytes, 0x224));
            assert_eq!(player.bytes, expected_player);
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original ten-profile initialization loop observations"]
    fn matches_native_ai_initialization() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_PROFILE_EVENTS")).unwrap();
        assert_eq!(&bytes[..8], b"MTAIPRFE");
        assert_eq!(
            (word(&bytes, 8), word(&bytes, 16), word(&bytes, 20)),
            (1, 10, 0x650)
        );
        let size = word(&bytes, 24) as usize;
        let mut at = 28;
        for case in 0..word(&bytes, 12) {
            let difficulty = word(&bytes, at) as i32;
            let refresh = word(&bytes, at + 4) != 0;
            let result = word(&bytes, at + 8) as u16;
            let event_count = word(&bytes, at + 16);
            let count_calls = word(&bytes, at + 24);
            at += 28;
            let mut profiles = vec![];
            let mut expected_profiles = vec![];
            let mut expected_players = vec![];
            let mut manager = Manager {
                bytes: [0; 0x28],
                players: vec![],
            };
            for _ in 0..11 {
                let profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
                at += 0x650;
                expected_profiles.push(bytes[at..at + 0x650].to_vec());
                at += 0x650;
                manager.players.push(Player {
                    reference: word(&profile, 0),
                    bytes: bytes[at..at + size].to_vec(),
                    auxiliaries: vec![],
                });
                at += size;
                expected_players.push(bytes[at..at + size].to_vec());
                at += size;
                profiles.push(profile);
            }
            let mut expected_events = vec![];
            for _ in 0..event_count {
                expected_events.push((word(&bytes, at) as usize, word(&bytes, at + 4)));
                at += 12;
            }
            let mut expected_count_calls = vec![];
            for _ in 0..count_calls {
                expected_count_calls.push((word(&bytes, at), word(&bytes, at + 8) as u16));
                at += 12;
            }
            let mut events = vec![];
            let mut counted = vec![];
            manager
                .initialize_ai(
                    &mut profiles,
                    difficulty,
                    refresh,
                    &mut |_, _, index, event| {
                        events.push((index, [0x550f60, 0x550f48, 0x550f30, 0x550f20][event]));
                        Ok(())
                    },
                    &mut |player, _| {
                        counted.push((player.reference, result));
                        Ok(result)
                    },
                )
                .unwrap();
            assert_eq!(events, expected_events, "events {case}");
            assert_eq!(counted, expected_count_calls, "optional count {case}");
            for index in 0..11 {
                assert_eq!(
                    profiles[index].as_slice(),
                    expected_profiles[index],
                    "profile {case}/{index}"
                );
                assert_eq!(
                    manager.players[index].bytes, expected_players[index],
                    "player {case}/{index}"
                );
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original48a1c0 constructor observations and v1.4 executable"]
    fn matches_native_common_constructor() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PLAYER_CONSTRUCTOR")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let name = image.at(0x541c80, 32).unwrap();
        assert_eq!(&bytes[..8], b"MTP48A1D");
        assert_eq!(word(&bytes, 8), 2);
        let mut at = 16;
        for case in 0..word(&bytes, 12) {
            let process_state = word(&bytes, at + 8);
            let existing = word(&bytes, at + 12) != 0;
            assert_eq!(word(&bytes, at + 16), 0x100f0000);
            let reference = word(&bytes, at + 20);
            at += 24;
            let globals: [u32; 5] = std::array::from_fn(|i| word(&bytes, at + i * 4));
            at += 20;
            let mut registry = RandomRegistry {
                process: [process_state, 0x343fd, 0x269ec3],
                head: if existing { 0x1005f000 } else { 0 },
                tail: if existing { 0x1005f000 } else { 0 },
            };
            let mut old_node = [0xcc; 0x30];
            put(&mut old_node, 0xc, 0);
            put(&mut old_node, 0x10, 0);
            assert_eq!(word(&bytes, at), 0);
            at += 4;
            let before_len = word(&bytes, at) as usize;
            let after_len = word(&bytes, at + 4) as usize;
            at += 8;
            let mut player = Player {
                reference,
                bytes: bytes[at..at + before_len].to_vec(),
                auxiliaries: vec![],
            };
            at += before_len;
            let expected = &bytes[at..at + after_len];
            at += after_len;
            assert_eq!(word(&bytes, at), 2);
            at += 4;
            let mut allocations = vec![];
            for _ in 0..2 {
                let reference = word(&bytes, at);
                assert_eq!(word(&bytes, at + 4), 0x30);
                let before_len = word(&bytes, at + 12) as usize;
                let after_len = word(&bytes, at + 16) as usize;
                at += 20;
                let before: [u8; 0x30] = bytes[at..at + before_len].try_into().unwrap();
                at += before_len;
                let after: [u8; 0x30] = bytes[at..at + after_len].try_into().unwrap();
                at += after_len;
                allocations.push((reference, before, after));
            }
            player
                .initialize_common(
                    name,
                    &mut || {
                        let mut account = Auxiliary {
                            reference: allocations[0].0,
                            bytes: allocations[0].1,
                        };
                        registry.initialize_account(&mut account, &mut |tail, next| {
                            assert!(existing);
                            assert_eq!(tail, 0x1005f000);
                            put(&mut old_node, 0xc, next);
                            Ok(())
                        })?;
                        Ok(account)
                    },
                    &mut || {
                        Ok(Some(Auxiliary {
                            reference: allocations[1].0,
                            bytes: allocations[1].1,
                        }))
                    },
                )
                .unwrap();
            assert_eq!(player.bytes, expected, "common player {case}");
            let account = crate::economy::PlayerAccount::from_runtime(&player.bytes).unwrap();
            assert_eq!(account.cash, 0.);
            assert_eq!(account.owned_categories, [0; 52]);
            assert_eq!(account.retail_counters, [0; 2]);
            assert_eq!(account.housing_counter, 0);
            assert_eq!(account.replenishment_modifier, 0);
            for event in 0..38 {
                assert_eq!(account.ledger(event), Some(0.));
            }
            for (actual, expected) in player.auxiliaries.iter().zip(allocations) {
                assert_eq!(actual.reference, expected.0);
                assert_eq!(actual.bytes, expected.2, "common auxiliary {case}");
            }
            let existing_node_length = word(&bytes, at) as usize;
            at += 4;
            if existing {
                assert_eq!(old_node.as_slice(), &bytes[at..at + existing_node_length]);
            } else {
                assert_eq!(existing_node_length, 0);
            }
            at += existing_node_length;
            assert_eq!(
                [
                    registry.head,
                    registry.tail,
                    registry.process[0],
                    registry.process[1],
                    registry.process[2]
                ],
                globals,
                "RNG globals {case}"
            );
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original player creation observations and v1.4 executable"]
    fn matches_native_creation() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PLAYER_CREATION")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let cash = Manager::initial_cash(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTPCRT01");
        assert_eq!(word(&bytes, 8), 1);
        let mut at = 16;
        for _ in 0..word(&bytes, 12) {
            let length = word(&bytes, at) as usize;
            let mask = word(&bytes, at + 4);
            let count = word(&bytes, at + 8);
            at += 12 + length;
            let mut manager = Manager {
                bytes: [0; 0x28],
                players: vec![],
            };
            for slot in 0..6 {
                manager.bytes[slot] = ((mask >> slot) & 1) as u8;
            }
            let mut profiles = vec![[0; 0x650]; 10];
            for case in 0..count {
                let kind = word(&bytes, at) as i32;
                let character = word(&bytes, at + 4) as i32;
                let result = word(&bytes, at + 8);
                let slot = word(&bytes, at + 12);
                let reenable = word(&bytes, at + 16);
                at += 20;
                for slot in 0..6 {
                    if reenable & (1 << slot) != 0 {
                        manager.bytes[slot] = 1;
                    }
                }
                assert_eq!(manager.bytes.as_slice(), &bytes[at..at + 0x28]);
                at += 0x28;
                let after = &bytes[at..at + 0x28];
                at += 0x28;
                let length = word(&bytes, at) as usize;
                at += 4;
                assert_eq!(length, 0x650);
                profiles[character as usize].copy_from_slice(&bytes[at..at + length]);
                at += length;
                let length = word(&bytes, at) as usize;
                at += 4;
                let expected_profile = &bytes[at..at + length];
                at += length;
                let length = word(&bytes, at) as usize;
                at += 4;
                let expected_player = &bytes[at..at + length];
                at += length;
                let mut allocations = 0;
                let actual = manager
                    .register(
                        &mut profiles,
                        Creation {
                            kind,
                            character,
                            name: b"NativePlayer",
                            initial_cash: cash,
                        },
                        &mut |_, kind| {
                            allocations += 1;
                            assert!(slot < 6);
                            // Match the native fixture's explicit constructor seam.
                            Ok(Player {
                                reference: word(after, 0x10 + slot as usize * 4),
                                bytes: vec![0; if kind == 0 { 0x224 } else { 0x1e50 }],
                                auxiliaries: vec![],
                            })
                        },
                    )
                    .unwrap();
                assert_eq!(u32::from(actual), result, "creation {case}");
                assert_eq!(allocations, usize::from(result == 1));
                assert_eq!(manager.bytes.as_slice(), after, "manager {case}");
                assert_eq!(
                    profiles[character as usize].as_slice(),
                    expected_profile,
                    "profile {case}"
                );
                if result == 1 {
                    let player = manager.players.last().unwrap();
                    let mut snapshot = player.bytes.clone();
                    snapshot.resize(length, 0);
                    assert_eq!(snapshot, expected_player, "player {case}");
                } else {
                    assert!(expected_player.is_empty());
                }
            }
        }
        assert_eq!(at, bytes.len());
    }

    /// A pause raised in the 0x480ec0 drain ends the batch before that
    /// iteration's 0x48cea0 player tick, so the record trails the sales of the
    /// ticks since the last copy. The save prefixes and the batch-end copy
    /// both publish the live accounts first, as the original's single record
    /// would read.
    #[test]
    fn pause_ended_batch_record_matches_live_sales_after_publish() {
        let mut manager = Manager::initialize([0; 0x28]);
        manager.players.push(Player {
            reference: 0x1000,
            bytes: vec![0; 0x13f8],
            auxiliaries: vec![],
        });
        let mut accounts =
            vec![crate::economy::PlayerAccount::from_runtime(&manager.players[0].bytes).unwrap()];
        let mut clock = crate::clock::Clock {
            calendar: [0; 5],
            timing: [5970, 5969, 0, 0, 0, 1],
        };
        let mut controls = crate::scheduler::Controls {
            iterations: 10,
            abort: false,
            alternate_script_active: false,
            network_enabled: false,
            network_override: false,
            deadline: u32::MAX,
            gate_32: false,
            gate_32_offset: false,
            before_six_sound: 0,
        };
        let mut player_ticks = 0;
        crate::scheduler::advance(&mut clock, &mut controls, |event, clock, controls| {
            match event {
                // A timed tip paused the game during tick 5974's work.
                crate::scheduler::Event::Helper(0x480ec0) => {
                    controls.abort = clock.timing[0] == 5975;
                }
                // One unit sold each normal tick (0x4625c0 adds it to +0x1b4).
                crate::scheduler::Event::Residents => accounts[0].retail_counters[1] += 1,
                crate::scheduler::Event::Helper(0x48cea0) => {
                    player_ticks += 1;
                    manager.publish_accounts(&accounts)?;
                }
                _ => {}
            }
            Ok(0)
        })
        .unwrap();
        assert_eq!(clock.timing[..2], [5975, 5973]);
        assert_eq!(player_ticks, 0, "the aborted iteration skipped 0x48cea0");
        assert_eq!(accounts[0].retail_counters[1], 4);
        assert_eq!(
            word(&manager.players[0].bytes, 0x1b4),
            0,
            "record trails before publish"
        );

        let prefixes = manager.account_prefixes(&accounts).unwrap();
        assert_eq!(
            word(&prefixes[0], 0x1b0),
            4,
            "save prefix +0x1b4 holds the live count"
        );

        manager.players[0].bytes[0x1b4..0x1b8].fill(0);
        manager.publish_accounts(&accounts).unwrap();
        assert_eq!(word(&manager.players[0].bytes, 0x1b4), 4);

        accounts.push(accounts[0].clone());
        assert!(manager.publish_accounts(&accounts).is_err());
    }
}
