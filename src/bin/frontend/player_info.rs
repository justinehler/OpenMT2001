//! Read-only original412810 financial report over saved player ledger banks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub style: u32,
    pub label: u32,
    pub amount: u32,
}
pub fn report(saved: &[u8; 440], today: bool) -> Result<Vec<Row>, String> {
    let current = usize::from(saved[0x140]);
    if current > 1 {
        return Err("saved player ledger selector leaves banks".into());
    }
    let bank = if today { current } else { 1 - current };
    let get = |runtime: usize| -> Result<f32, String> {
        let at = runtime - 4 + bank * 152;
        let value = f32::from_le_bytes(saved[at..at + 4].try_into().unwrap());
        if !value.is_finite() {
            return Err("non-finite saved report value".into());
        }
        Ok(value)
    };
    let add = |a: f32, b: f32| (f64::from(a) + f64::from(b)) as f32;
    let sum = |offsets: &[usize]| -> Result<f32, String> {
        let mut value = openmt2001::x87::Sum::from_f32(get(offsets[0])?)?;
        if !today {
            value.add_f32(0.)?;
        }
        for &at in &offsets[1..] {
            value.add_f32(get(at)?)?;
        }
        Ok(value.to_f32())
    };
    let mut rows = vec![Row {
        style: 1,
        label: 0x25,
        amount: 0,
    }];
    let mut append = |label: u32, value: f32, always: bool| {
        if always || value != 0. {
            rows.push(Row {
                style: 0,
                label,
                amount: value.to_bits(),
            });
        }
    };
    for (label, offset) in [(0x26, 0xa0), (0x486, 0x9c), (0x5bb, 0x98)] {
        append(label, get(offset)?, true);
    }
    append(0x50, sum(&[0x78, 0x80, 0x7c, 0x84])?, false);
    append(0x4f, sum(&[0x88, 0x8c, 0x90, 0x94])?, false);
    for (label, offset) in [(0x27, 0x70), (0x5b, 0x6c), (0x708, 0x74), (0x5c, 0xa4)] {
        append(label, get(offset)?, false);
    }
    let mut income = 0.;
    for category in 22..37 {
        income = add(income, get(0x14 + category * 4)?);
    }
    rows.push(Row {
        style: 2,
        label: 0,
        amount: 0,
    });
    rows.push(Row {
        style: 0,
        label: 0x3f,
        amount: income.to_bits(),
    });
    rows.push(Row {
        style: 3,
        label: 0,
        amount: 0,
    });
    rows.push(Row {
        style: 1,
        label: 0x43,
        amount: 0,
    });
    let mut append = |label: u32, value: f32, always: bool| {
        if always || value != 0. {
            rows.push(Row {
                style: 0,
                label,
                amount: (-value).to_bits(),
            });
        }
    };
    append(0x4a, get(0x58)?, true);
    append(0x49, get(0x4c)?, true);
    append(0x50, sum(&[0x24, 0x2c, 0x28, 0x30])?, false);
    append(0x28, get(0x14)?, false);
    append(
        0x5bc,
        (f64::from(get(0x44)?) + f64::from(get(0x54)?) + f64::from(get(0x18)?)) as f32,
        false,
    );
    for (label, offset) in [(0x5b, 0x20), (0x5c, 0x5c), (0x5bd, 0x60)] {
        append(label, get(offset)?, false);
    }
    let mut costs = 0.;
    for category in 0..21 {
        costs = add(costs, get(0x14 + category * 4)?);
    }
    rows.push(Row {
        style: 2,
        label: 0,
        amount: 0,
    });
    rows.push(Row {
        style: 0,
        label: 0x40,
        amount: (-costs).to_bits(),
    });
    Ok(rows)
}
/// Original40f1d0: active current player first, then active roster slots.
pub fn player_order(active: &[bool], current: Option<usize>) -> Vec<usize> {
    let first = current.filter(|&i| active.get(i) == Some(&true));
    first
        .into_iter()
        .chain(
            active
                .iter()
                .enumerate()
                .filter_map(|(i, &on)| (on && Some(i) != first).then_some(i)),
        )
        .collect()
}
/// Original40ff70 uses13% of the button extent, not absolute coordinates.
pub fn portrait_inset(bounds: [f32; 4]) -> Result<[f32; 4], String> {
    use openmt2001::x87::Sum;
    let mut out = bounds;
    for axis in 0..2 {
        let mut extent = Sum::from_f32(bounds[axis + 2])?;
        extent.add(Sum::from_f32(bounds[axis])?.negated());
        extent.mul(Sum::from_f64(0.13)?);
        let mut left = Sum::from_f32(bounds[axis])?;
        left.add(extent);
        let mut right = Sum::from_f32(bounds[axis + 2])?;
        right.add(extent.negated());
        out[axis] = left.to_f32();
        out[axis + 2] = right.to_f32();
    }
    Ok(out)
}
/// Original40dd0a..40df24 period controls; texture97 supplies both extents.
pub fn period_layout(bounds: [f32; 4], dimensions: [u32; 2]) -> [[f32; 4]; 3] {
    let width = dimensions[0] as f32;
    let height = dimensions[1] as f32;
    let y = bounds[1] + 45.;
    let left = bounds[0] + width;
    let right = bounds[2] - width - width - width;
    [
        [left, y, left + width, y + height],
        [right, y, right + width, y + height],
        [left, y, bounds[2] - width - width, y + height],
    ]
}
/// A live copy of the original runtime+4 financial prefixes. The simulation
/// remains the owner of the player and account records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub records: Vec<[u8; 440]>,
    pub current: Option<usize>,
}
impl Snapshot {
    pub fn from_runtime(
        manager: &openmt2001::player_manager::Manager,
        accounts: &[openmt2001::economy::PlayerAccount],
        current: u32,
    ) -> Result<Self, String> {
        if manager.players.len() != accounts.len() || accounts.len() > 6 {
            return Err("financial roster/account count mismatch".into());
        }
        let records = manager
            .players
            .iter()
            .zip(accounts)
            .map(|(player, account)| {
                let mut bytes = player.bytes.clone();
                account.write_runtime(&mut bytes)?;
                bytes
                    .get(4..444)
                    .ok_or("truncated financial runtime player")?
                    .try_into()
                    .map_err(|_| "financial player prefix length".into())
            })
            .collect::<Result<Vec<[u8; 440]>, String>>()?;
        let current_index = manager.players.iter().position(|p| p.reference == current);
        if current != 0 && current_index.is_none() {
            return Err("current financial player leaves roster".into());
        }
        Ok(Self {
            records,
            current: current_index,
        })
    }
}
pub struct State {
    /// Shared runtime+4 prefix, retained independently of scene save metadata.
    pub records: Vec<[u8; 440]>,
    pub current: Option<usize>,
    pub fresh: bool,
    pub players: Vec<usize>,
    pub selected: usize,
    pub today: bool,
    pub scroll: usize,
    pub rows: Vec<Row>,
}
impl State {
    pub fn new(players: &[[u8; 440]]) -> Result<Self, String> {
        let mut state = Self {
            records: players.to_vec(),
            current: None,
            fresh: false,
            players: players
                .iter()
                .enumerate()
                .filter_map(|(i, p)| (p[0xc] == 1).then_some(i))
                .collect(),
            selected: 0,
            today: true,
            scroll: 0,
            rows: vec![],
        };
        state.refresh()?;
        Ok(state)
    }
    pub fn from_runtime(
        manager: &openmt2001::player_manager::Manager,
        accounts: &[openmt2001::economy::PlayerAccount],
        current: u32,
    ) -> Result<Self, String> {
        let snapshot = Snapshot::from_runtime(manager, accounts, current)?;
        let mut state = Self::new(&snapshot.records)?;
        state.fresh = true;
        state.current = snapshot.current;
        if let Some(position) = state.players.iter().position(|p| Some(*p) == state.current) {
            state.selected = position;
            state.refresh()?;
        }
        Ok(state)
    }
    /// Refresh 412810's report from a retained session without resetting the
    /// user's selected roster slot, period, or scroll position.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) -> Result<bool, String> {
        if self.records == snapshot.records && self.current == snapshot.current {
            return Ok(false);
        }
        let selected_slot = self.players.get(self.selected).copied();
        let players = snapshot
            .records
            .iter()
            .enumerate()
            .filter_map(|(i, p)| (p[0xc] == 1).then_some(i))
            .collect::<Vec<_>>();
        let selected = selected_slot
            .and_then(|slot| players.iter().position(|&p| p == slot))
            .or_else(|| {
                snapshot
                    .current
                    .and_then(|slot| players.iter().position(|&p| p == slot))
            })
            .unwrap_or(0);
        let rows = if let Some(&slot) = players.get(selected) {
            report(&snapshot.records[slot], self.today)?
        } else {
            Vec::new()
        };
        self.records = snapshot.records;
        self.current = snapshot.current;
        self.players = players;
        self.selected = selected;
        self.rows = rows;
        self.scroll = self.scroll.min(self.rows.len().saturating_sub(19));
        Ok(true)
    }
    pub fn refresh(&mut self) -> Result<(), String> {
        self.rows = if let Some(&index) = self.players.get(self.selected) {
            report(&self.records[index], self.today)?
        } else {
            vec![]
        };
        self.scroll = 0;
        Ok(())
    }
    pub fn name(&self) -> Result<Vec<u8>, String> {
        let Some(&index) = self.players.get(self.selected) else {
            return Ok(b"No active players".iter().map(|b| b - 32).collect());
        };
        let bytes = &self.records[index][0x154..];
        let end = bytes
            .iter()
            .position(|b| *b == 0)
            .ok_or("unterminated saved player name")?;
        bytes[..end]
            .iter()
            .map(|b| {
                if (32..=126).contains(b) {
                    Ok(b - 32)
                } else {
                    Err("saved player name needs an unrecovered glyph mapping".into())
                }
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_snapshot_preserves_selection_period_and_scroll() {
        fn record() -> [u8; 440] {
            let mut saved = [0; 440];
            saved[0xc] = 1;
            for bank in 0..2 {
                for offset in [
                    0x14, 0x18, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x44, 0x4c, 0x54, 0x58, 0x5c, 0x60,
                    0x6c, 0x70, 0x74, 0x78, 0x7c, 0x80, 0x84, 0x88, 0x8c, 0x90, 0x94, 0x98, 0x9c,
                    0xa0, 0xa4,
                ] {
                    let at = offset - 4 + bank * 152;
                    saved[at..at + 4].copy_from_slice(&1f32.to_le_bytes());
                }
            }
            saved
        }
        let mut state = State::new(&[record(), record()]).unwrap();
        state.fresh = true;
        state.current = Some(0);
        state.selected = 1;
        state.today = false;
        state.refresh().unwrap();
        assert!(state.rows.len() > 20);
        state.scroll = 2;
        let mut records = vec![record(), record()];
        let prior_bank_amount = 0xa0 - 4 + 152;
        records[1][prior_bank_amount..prior_bank_amount + 4].copy_from_slice(&2f32.to_le_bytes());
        let snapshot = Snapshot {
            records,
            current: Some(0),
        };
        assert!(state.apply_snapshot(snapshot.clone()).unwrap());
        assert_eq!(state.selected, 1);
        assert!(!state.today);
        assert_eq!(state.scroll, 2);
        assert!(
            state
                .rows
                .iter()
                .any(|r| r.label == 0x26 && r.amount == 2f32.to_bits())
        );
        assert!(!state.apply_snapshot(snapshot.clone()).unwrap());
        let mut eliminated = snapshot;
        eliminated.records[1][0xc] = 0;
        assert!(state.apply_snapshot(eliminated).unwrap());
        assert_eq!(state.players, vec![0]);
        assert_eq!(state.selected, 0);
        assert!(!state.today);
    }
    #[test]
    fn runtime_snapshot_uses_authoritative_account_without_mutating_identity() {
        let mut bytes = vec![0; 444];
        bytes[0x10] = 1;
        bytes[0x150..0x154].copy_from_slice(&4u32.to_le_bytes());
        bytes[0x158..0x15d].copy_from_slice(b"Human");
        let mut account = openmt2001::economy::PlayerAccount::from_runtime(&bytes).unwrap();
        account.post(125., 35).unwrap();
        account.rotate_ledger();
        account.post(40., 35).unwrap();
        let original = bytes.clone();
        let manager = openmt2001::player_manager::Manager {
            bytes: [0; 0x28],
            players: vec![openmt2001::player_manager::Player {
                reference: 123,
                bytes,
                auxiliaries: vec![],
            }],
        };
        let mut state = State::from_runtime(&manager, &[account], 123).unwrap();
        assert_eq!(state.current, Some(0));
        assert_eq!(state.name().unwrap(), b"Human".map(|b| b - 32));
        assert_eq!(
            f32::from_le_bytes(state.records[0][..4].try_into().unwrap()),
            165.
        );
        assert!(
            state
                .rows
                .iter()
                .any(|r| r.label == 0x26 && r.amount == 40f32.to_bits())
        );
        state.today = false;
        state.refresh().unwrap();
        assert!(
            state
                .rows
                .iter()
                .any(|r| r.label == 0x26 && r.amount == 125f32.to_bits())
        );
        assert_eq!(manager.players[0].bytes, original);
    }
    #[test]
    #[ignore = "private original player button/order fixture"]
    fn matches_native_player_style() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_PLAYER_STYLE")).unwrap();
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let word_at = |a| u32::from_le_bytes(image.at(a, 4).unwrap().try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTPLRSTY");
        let mut at = 8;
        fn word(b: &[u8], a: &mut usize) -> u32 {
            let v = u32::from_le_bytes(b[*a..*a + 4].try_into().unwrap());
            *a += 4;
            v
        }
        for _ in 0..word(&bytes, &mut at) {
            let active: [bool; 6] = std::array::from_fn(|_| word(&bytes, &mut at) == 1);
            let current = word(&bytes, &mut at) as i32;
            let order = player_order(&active, usize::try_from(current).ok());
            assert_eq!(order.len(), word(&bytes, &mut at) as usize);
            for slot in order {
                assert_eq!(slot, word(&bytes, &mut at) as usize);
                let character = word(&bytes, &mut at);
                let bounds = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
                let expected: [u32; 4] = std::array::from_fn(|_| word(&bytes, &mut at));
                assert_eq!(portrait_inset(bounds).unwrap().map(f32::to_bits), expected);
                let base = word_at(0x541b0c + character * 40);
                assert_eq!(
                    [
                        base,
                        base + 2,
                        base + 1,
                        base + 3,
                        word_at(0x541af8 + character * 40)
                    ],
                    std::array::from_fn(|_| word(&bytes, &mut at))
                );
            }
        }
        for _ in 0..word(&bytes, &mut at) {
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
            let dimensions = std::array::from_fn(|_| word(&bytes, &mut at));
            for actual in period_layout(bounds, dimensions) {
                let expected: [u32; 4] = std::array::from_fn(|_| word(&bytes, &mut at));
                assert_eq!(actual.map(f32::to_bits), expected);
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original saved/synthetic financial report oracle"]
    fn matches_native_player_report() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_PLAYER_REPORT")).unwrap();
        assert_eq!(&bytes[..8], b"MTPRPT01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let count = word(8);
        let mut at = 12;
        for case in 0..count {
            let player = bytes[at..at + 440].try_into().unwrap();
            at += 440;
            let mode = word(at);
            let count = word(at + 4);
            at += 8;
            let mut expected = vec![];
            for _ in 0..count {
                expected.push(Row {
                    style: word(at),
                    label: word(at + 4),
                    amount: word(at + 8),
                });
                at += 12;
            }
            assert_eq!(report(&player, mode == 1).unwrap(), expected, "case{case}");
        }
        assert_eq!(at, bytes.len());
    }
}
