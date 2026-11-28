//! Original004bb500 six-series history sampler and tag20000 storage.
//! Host history charts read these same rings; partial writes survive errors.

pub const RECORD_BYTES: usize = 0x10f44;
pub const SAMPLES: usize = 480;

pub struct State {
    bytes: Vec<u8>,
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
    #[ignore = "requires original004bb500 history/cache observations"]
    fn matches_native_history() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_HISTORY")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTHIST02");
        assert_eq!(r.word(), 2);
        let cases = r.word();
        for _ in 0..cases {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let count = r.word() as usize;
            let calls = r.word();
            let flags = r.take(4);
            assert_eq!(flags[1], 0, "{name}");
            r.take(count * 9); // active bytes, controller kinds, integer values
            let raw: Vec<_> = (0..count).map(|_| r.blob()).collect();
            let players: Vec<[u8; 440]> =
                (0..count).map(|_| r.blob().try_into().unwrap()).collect();
            for (raw, prefix) in raw.iter().zip(&players) {
                assert_eq!(&raw[4..444], prefix);
            }
            let categories: Vec<[u8; 380]> = r
                .blob()
                .chunks_exact(380)
                .map(|x| x.try_into().unwrap())
                .collect();
            let buildings: Vec<[u8; 916]> = r
                .blob()
                .chunks_exact(916)
                .map(|x| x.try_into().unwrap())
                .collect();
            let businesses: Vec<[u8; 544]> = r
                .blob()
                .chunks_exact(544)
                .map(|x| x.try_into().unwrap())
                .collect();
            let cache_before = r.blob();
            let cache_after = r.blob();
            let initial: Vec<_> = (0..6).map(|_| r.blob()).collect();
            let expected: Vec<_> = (0..6).map(|_| r.blob()).collect();
            let mut history = State::default();
            for (series, region) in initial.iter().enumerate() {
                for player in 0..6 {
                    let source = (0x3c0 + player * 0xb40) * 4;
                    let target = (player * 6 + series) * SAMPLES * 4;
                    history.bytes[target..target + SAMPLES * 4]
                        .copy_from_slice(&region[source..source + SAMPLES * 4]);
                }
            }
            for series in 0..6 {
                for base in [0x10e00, 0x10e90] {
                    for player in 0..count {
                        history.put(base + (player * 6 + series) * 4, r.word());
                    }
                }
            }
            let mut expected_state = Vec::new();
            for _ in 0..6 * count * 2 {
                expected_state.push(r.word());
            }
            let trace_count = r.word();
            let expected_trace: Vec<_> = (0..trace_count).map(|_| (r.word(), r.word())).collect();
            let word = |bytes: &[u8], at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            let mut cache = crate::player_metrics::BusinessCounts {
                age: cache_before[3],
                values: std::array::from_fn(|p| word(cache_before, 0x90 + p * 16) as i32),
            };
            let categories: [[u8; 380]; 52] = categories.try_into().unwrap();
            let chains =
                crate::building_pool::category_chains(&categories, &buildings, 0x591318).unwrap();
            let records: Vec<_> = chains
                .iter()
                .enumerate()
                .flat_map(|(category, chain)| {
                    let buildings = &buildings;
                    chain
                        .iter()
                        .map(move |&slot| crate::player_metrics::Building {
                            category,
                            slot,
                            bytes: &buildings[slot],
                        })
                })
                .collect();
            let owners: Vec<_> = categories
                .iter()
                .map(|row| (row[5], f32::from_bits(word(row, 0x84))))
                .collect();
            let business_views: Vec<_> = businesses.iter().map(|row| row.as_slice()).collect();
            let values = crate::player_metrics::comparison(
                &players,
                &business_views,
                &records,
                &owners,
                &image,
                9,
            )
            .unwrap();
            let mut trace = Vec::new();
            for _ in 0..calls {
                history
                    .sample(&players, |metric, player| {
                        let row = &values.iter().find(|(p, _)| *p == player).unwrap().1;
                        let (address, value) = match metric {
                            Metric::EmpireValue => (0x47fbb0, Value::Float(row[4])),
                            Metric::BusinessCount => (
                                0x47ff20,
                                Value::Integer(cache.get(player, count, &businesses)?),
                            ),
                            Metric::CurrentLedgerIncome => (0x47fca0, Value::Float(row[3])),
                        };
                        trace.push((address, player as u32));
                        Ok(value)
                    })
                    .unwrap();
            }
            assert_eq!(
                trace,
                expected_trace
                    .into_iter()
                    .filter(|(a, _)| *a != 0x48a580)
                    .collect::<Vec<_>>(),
                "{name}"
            );
            assert_eq!(cache.age, cache_after[3], "{name}");
            for player in 0..6 {
                assert_eq!(
                    cache.values[player] as u32,
                    word(cache_after, 0x90 + player * 16),
                    "{name}"
                );
            }
            let mut state = Vec::new();
            for (series, expected_ring) in expected.iter().enumerate() {
                for player in 0..count {
                    state.push(history.cursor(player, series).unwrap());
                }
                for player in 0..count {
                    state.push(history.count(player, series).unwrap());
                }
                for player in 0..6 {
                    let start = (0x3c0 + player * 0xb40) * 4;
                    assert_eq!(
                        history.ring(player, series).unwrap(),
                        &expected_ring[start..start + SAMPLES * 4],
                        "{name} player{player} series{series}"
                    );
                }
            }
            assert_eq!(state, expected_state, "{name}");
        }
        assert!(r.0.is_empty());
    }
}
impl Default for State {
    fn default() -> Self {
        Self {
            bytes: vec![0; RECORD_BYTES],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    EmpireValue,
    BusinessCount,
    CurrentLedgerIncome,
}
#[derive(Clone, Copy, Debug)]
pub enum Value {
    Float(f32),
    Integer(i32),
}

impl State {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != RECORD_BYTES {
            return Err("invalid player history record size".into());
        }
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn word(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }
    fn put(&mut self, at: usize, value: u32) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    pub fn cursor(&self, player: usize, series: usize) -> Option<u32> {
        (player < 6 && series < 6).then(|| self.word(0x10e00 + (player * 6 + series) * 4))
    }
    pub fn count(&self, player: usize, series: usize) -> Option<u32> {
        (player < 6 && series < 6).then(|| self.word(0x10e90 + (player * 6 + series) * 4))
    }
    pub fn ring(&self, player: usize, series: usize) -> Option<&[u8]> {
        if player >= 6 || series >= 6 {
            return None;
        }
        let start = (player * 6 + series) * SAMPLES * 4;
        Some(&self.bytes[start..start + SAMPLES * 4])
    }
    fn append(&mut self, player: usize, series: usize, value: f32) -> Result<(), String> {
        let state = (player * 6 + series) * 4;
        let cursor = self.word(0x10e00 + state);
        if cursor >= SAMPLES as u32 {
            return Err("history cursor leaves original ring".into());
        }
        self.put(
            (player * 6 + series) * SAMPLES * 4 + cursor as usize * 4,
            value.to_bits(),
        );
        self.put(0x10e00 + state, (cursor + 1) % SAMPLES as u32);
        let count = self.word(0x10e90 + state).wrapping_add(1) as i32;
        self.put(0x10e90 + state, count.min(SAMPLES as i32) as u32);
        Ok(())
    }

    /// Prefixes begin after the runtime vtable, matching saved player records.
    /// The three getters are evaluated in original order for each active slot.
    /// BusinessCount must use its live cache (0047ff20), not a GUI recount.
    pub fn sample(
        &mut self,
        players: &[[u8; 440]],
        mut metric: impl FnMut(Metric, usize) -> Result<Value, String>,
    ) -> Result<(), String> {
        if players.len() > 6 {
            return Err("history roster exceeds six slots".into());
        }
        let int = |p: &[u8], at| i32::from_le_bytes(p[at..at + 4].try_into().unwrap());
        let share = |p: &[u8]| int(p, 0x1ac).wrapping_add(int(p, 0x1a8)).max(0);
        let total = players
            .iter()
            .filter(|p| p[0xc] != 0)
            .fold(0i32, |sum, p| sum.wrapping_add(share(p)));
        for (index, player) in players.iter().enumerate() {
            if player[0xc] == 0 {
                continue;
            }
            self.append(index, 0, int(player, 0x1b0) as f32)?;
            let Value::Float(empire) = metric(Metric::EmpireValue, index)? else {
                return Err("history empire getter returned an integer".into());
            };
            self.append(index, 1, empire)?;
            self.append(index, 2, f32::from_bits(int(player, 0) as u32))?;
            let Value::Integer(businesses) = metric(Metric::BusinessCount, index)? else {
                return Err("history business getter returned a float".into());
            };
            self.append(index, 3, businesses as f32)?;
            let Value::Float(income) = metric(Metric::CurrentLedgerIncome, index)? else {
                return Err("history income getter returned an integer".into());
            };
            self.append(index, 4, income)?;
            if total != 0 {
                let amount = share(player)
                    .wrapping_mul(100)
                    .checked_div(total)
                    .ok_or("original history share division overflow")?;
                self.append(index, 5, amount as f32)?;
            }
        }
        Ok(())
    }
}
