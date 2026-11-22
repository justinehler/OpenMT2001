//! Scenario objective manager86ce60, distinct from resident objectives.
pub struct State {
    pub kind: i32,
    pub target: f32,
    pub flag: u8,
    /// First byte of each eight-byte player row at manager+0c, read by4895b0.
    pub cash_statuses: [u8; 6],
    pub reached: [u8; 6],
    pub excess: [i32; 6],
    /// Three header and two per-player padding bytes, preserved by tag10000.
    pub reserved: [u8; 15],
}
impl Default for State {
    fn default() -> Self {
        Self {
            kind: 0,
            target: 0.,
            flag: 1,
            cash_statuses: [0; 6],
            reached: [0; 6],
            excess: [0; 6],
            reserved: [0; 15],
        }
    }
}
impl State {
    /// Original489460 constructor and489490 fresh-level reset. Padding and
    /// per-player excess values retain their prior bytes; result flags reset.
    pub fn reset(&mut self) {
        self.kind = 0;
        self.target = 0.;
        self.flag = 1;
        self.cash_statuses.fill(0);
        self.reached.fill(0);
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 60 {
            return Err("objective descriptor must contain60 bytes".into());
        }
        let mut state = Self {
            kind: i32::from_le_bytes(bytes[..4].try_into().unwrap()),
            target: f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            flag: bytes[8],
            ..Self::default()
        };
        state.reserved[..3].copy_from_slice(&bytes[9..12]);
        for slot in 0..6 {
            let at = 12 + slot * 8;
            state.cash_statuses[slot] = bytes[at];
            state.reached[slot] = bytes[at + 1];
            state.reserved[3 + slot * 2..5 + slot * 2].copy_from_slice(&bytes[at + 2..at + 4]);
            state.excess[slot] = i32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap());
        }
        Ok(state)
    }
    pub fn to_bytes(&self) -> [u8; 60] {
        let mut bytes = [0; 60];
        bytes[..4].copy_from_slice(&self.kind.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.target.to_le_bytes());
        bytes[8] = self.flag;
        bytes[9..12].copy_from_slice(&self.reserved[..3]);
        for slot in 0..6 {
            let at = 12 + slot * 8;
            bytes[at] = self.cash_statuses[slot];
            bytes[at + 1] = self.reached[slot];
            bytes[at + 2..at + 4].copy_from_slice(&self.reserved[3 + slot * 2..5 + slot * 2]);
            bytes[at + 4..at + 8].copy_from_slice(&self.excess[slot].to_le_bytes());
        }
        bytes
    }
    /// Original4dac40 after Lua argument conversion. Missing/non-numeric third
    /// arguments leave the byte untouched; unordered x87 comparison writes0.
    pub fn set(&mut self, kind: f64, target: f64, flag: Option<f64>) {
        self.kind = crate::economy::ftol_low_word(kind);
        self.target = target as f32;
        if let Some(value) = flag {
            self.flag = u8::from(value != 0. && !value.is_nan());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original4dac40 observations"]
    fn matches_original_objective_stores() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_OBJECTIVE").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTOBJ001");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 12 + count * 38);
        for row in bytes[12..].chunks_exact(38) {
            let number = |at| f64::from_le_bytes(row[at..at + 8].try_into().unwrap());
            let mut state = super::State {
                kind: -7,
                target: -9.,
                flag: row[28],
                cash_statuses: [0xa5; 6],
                ..super::State::default()
            };
            state.set(number(0), number(8), (row[24] != 0).then(|| number(16)));
            assert_eq!(state.cash_statuses, [0xa5; 6]);
            assert_eq!(state.kind.to_le_bytes(), row[29..33]);
            assert_eq!(state.target.to_bits().to_le_bytes(), row[33..37]);
            assert_eq!(state.flag, row[37]);
        }
    }
}
