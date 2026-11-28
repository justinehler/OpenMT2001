//! Original004b0810 construction/demolition transition pool. Building IDs and
//! category IDs are distinct; renderer, ownership and list removal are explicit
//! synchronous boundaries over the same live records.

pub const CAPACITY: usize = 128;
pub type Building = [u8; 0x394];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub count: u32,
    /// category, building, kind, stage, progress, completion payload, owner.
    /// Unwritten words survive both slot reuse and completion, as in v1.4.
    pub records: [[u32; 7]; CAPACITY],
    pub counters: [u32; 4],
}
impl Default for State {
    fn default() -> Self {
        Self {
            count: 0,
            records: [[0; 7]; CAPACITY],
            counters: [0; 4],
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Controls {
    pub network_enabled: bool,
    pub network_override: bool,
    pub effect_model_reference: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Original4b0750, with position at building+58 and fixed capacity 2.
    /// Each stage contributes to a two-nearest spatial construction sound
    /// list; 4adb10/4afcde consume the four lists through 4ad8b0.
    Progress { group: usize, building: usize },
    /// Original48ad80: ECX resolved through the one-based player table;
    /// stack arguments are payload, building ID,1.
    Complete {
        owner: u32,
        payload: u32,
        building_id: u32,
    },
    /// Original445d10: category receiver, building ID argument.
    Remove { category: u32, building_id: u32 },
    /// Model definition+ b4 resolved from the building's auxiliary+0 handle.
    /// Return the f32 bits; this lookup is read-only in the original.
    ModelHeight { building: usize },
    /// Original4eb520 stack arguments in order; return its object reference.
    CreateEffect { arguments: [u32; 6] },
}
fn word(row: &Building, at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}
fn put(row: &mut Building, at: usize, value: u32) {
    row[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn number(row: &Building, at: usize) -> f64 {
    f64::from(f32::from_bits(word(row, at)))
}
impl State {
    /// Original save chunk0x800 contains the count and all128 raw records.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 0xe04 {
            return Err("pending-world blob must contain0xe04 bytes".into());
        }
        let words: Vec<_> = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        Ok(Self {
            count: words[0],
            records: std::array::from_fn(|i| words[1 + i * 7..1 + (i + 1) * 7].try_into().unwrap()),
            counters: [0; 4],
        })
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        std::iter::once(self.count)
            .chain(self.records.iter().flatten().copied())
            .flat_map(u32::to_le_bytes)
            .collect()
    }
    /// Original4b0e60. A full native pool dereferences null; report it instead.
    pub fn construct(
        &mut self,
        category: u32,
        building: &mut Building,
        payload: u32,
        owner: u32,
    ) -> Result<(), String> {
        let slot = self.free()?;
        self.count = self.count.wrapping_add(1);
        let row = &mut self.records[slot];
        row[0] = category;
        row[1] = word(building, 0);
        row[2] = 1;
        row[3] = 0;
        row[5] = payload;
        row[6] = owner;
        for at in [0x70, 0x74, 0x78, 0x14] {
            put(building, at, 0);
        }
        Ok(())
    }
    /// Original4b0dd0 only admits a completed (status4) building.
    pub fn demolish(&mut self, category: u32, building: &mut Building) -> Result<bool, String> {
        if word(building, 0x14) != 4 {
            return Ok(false);
        }
        let slot = self.free()?;
        self.count = self.count.wrapping_add(1);
        self.records[slot][0] = category;
        self.records[slot][1] = word(building, 0);
        self.records[slot][2] = 2;
        self.records[slot][3] = 0;
        put(building, 0x14, 0);
        Ok(true)
    }
    fn free(&self) -> Result<usize, String> {
        self.records
            .iter()
            .position(|r| r[2] == 0)
            .ok_or_else(|| "original pending-world pool capacity exceeded".into())
    }
    fn clear(&mut self, slot: usize) {
        for i in [0, 1, 2, 4] {
            self.records[slot][i] = 0;
        }
        self.count = self.count.wrapping_sub(1);
    }
    pub fn advance(
        &mut self,
        buildings: &mut [Building],
        controls: &mut Controls,
        mut callback: impl FnMut(
            Event,
            &mut Self,
            &mut [Building],
            &mut Controls,
        ) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.counters.fill(0);
        for slot in 0..CAPACITY {
            if self.count == 0 {
                break;
            }
            for kind in [1, 2] {
                // Original reloads kind after its kind1 branch; callbacks may
                // replace the live record before the kind2 branch is tested.
                if self.records[slot][2] != kind {
                    continue;
                }
                let building = self.records[slot][1] as usize;
                let row = buildings
                    .get_mut(building)
                    .ok_or("pending building index leaves pool")?;
                let limit = u32::from(row[8]) * 25;
                let progress = self.records[slot][4] as i32;
                let event = match (kind, self.records[slot][3]) {
                    (_, 0) if progress < limit as i32 => {
                        self.records[slot][4] = (progress as u32).wrapping_add(1);
                        Some(Event::Progress { group: 0, building })
                    }
                    (1, 0) => {
                        self.records[slot][3] = 3;
                        put(row, 0x14, 1);
                        None
                    }
                    (2, 0) => {
                        put(row, 0x14, 3);
                        self.records[slot][3] = 2;
                        let width = f64::from(i32::from(row[6]) - i32::from(row[4]) + 1) * 10.;
                        let depth = f64::from(i32::from(row[7]) - i32::from(row[5]) + 1) * 10.;
                        // Width/depth are small exact integers even for inverted
                        // byte bounds. Their product with the f32 constant is exact.
                        Some(Event::CreateEffect {
                            arguments: [
                                controls.effect_model_reference,
                                ((number(row, 0xc) - width * 0.5) as f32).to_bits(),
                                ((number(row, 0x10) - depth * 0.5) as f32).to_bits(),
                                (depth as f32).to_bits(),
                                (depth as f32).to_bits(),
                                crate::economy::ftol_low_word(depth * width * f64::from(0.3_f32))
                                    as u32,
                            ],
                        })
                    }
                    (_, 1) if progress >= 0 => {
                        self.records[slot][4] = (progress as u32).wrapping_sub(1);
                        Some(Event::Progress { group: 1, building })
                    }
                    (1, 1) => {
                        put(row, 0x14, 4);
                        if (!controls.network_enabled || controls.network_override)
                            && self.records[slot][6] != 0
                        {
                            let event = Event::Complete {
                                owner: self.records[slot][6],
                                payload: self.records[slot][5],
                                building_id: word(row, 0),
                            };
                            callback(event, self, buildings, controls)?;
                        }
                        self.clear(slot);
                        None
                    }
                    (2, 1) => {
                        callback(
                            Event::Remove {
                                category: self.records[slot][0],
                                building_id: word(row, 0),
                            },
                            self,
                            buildings,
                            controls,
                        )?;
                        // Original reloads the building ID after the callback.
                        let index = self.records[slot][1] as usize;
                        self.clear(slot);
                        put(
                            buildings
                                .get_mut(index)
                                .ok_or("removed building leaves pool")?,
                            0x14,
                            5,
                        );
                        None
                    }
                    (1, 3) => {
                        for at in [0x70, 0x74, 0x78] {
                            put(
                                row,
                                at,
                                ((number(row, at) + f64::from(0.01_f32)) as f32).to_bits(),
                            );
                        }
                        if number(row, 0x74) >= 1. {
                            self.records[slot][3] = 1;
                            self.records[slot][4] = limit;
                            put(row, 0x14, 2);
                            for at in [0x70, 0x74, 0x78] {
                                put(row, at, 1_f32.to_bits());
                            }
                            None
                        } else {
                            Some(Event::Progress { group: 2, building })
                        }
                    }
                    (2, 2) => {
                        put(
                            row,
                            0x5c,
                            ((number(row, 0x5c) - f64::from(0.3_f32)) as f32).to_bits(),
                        );
                        let height = f32::from_bits(callback(
                            Event::ModelHeight { building },
                            self,
                            buildings,
                            controls,
                        )?);
                        let row = buildings
                            .get_mut(building)
                            .ok_or("pending building disappeared")?;
                        let combined = number(row, 0x5c) + f64::from(height);
                        if combined < f64::from(-10.3_f32) || combined.is_nan() {
                            put(row, 0x14, 2);
                            self.records[slot][3] = 1;
                            self.records[slot][4] = u32::from(row[8]) * 25;
                            None
                        } else {
                            Some(Event::Progress { group: 3, building })
                        }
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    let value = callback(event, self, buildings, controls)?;
                    if matches!(event, Event::CreateEffect { .. }) {
                        put(&mut buildings[building], 0x7c, value);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
    }
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let p = self.at;
            self.at += n;
            &self.bytes[p..self.at]
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn name(&mut self) -> String {
            let n = u16::from_le_bytes(self.take(2).try_into().unwrap());
            String::from_utf8(self.take(n as usize).to_vec()).unwrap()
        }
        fn json(&mut self) {
            let n = self.word();
            self.take(n as usize);
        }
        fn buildings(&mut self) -> Vec<Building> {
            self.take(600 * 916)
                .chunks_exact(916)
                .map(|r| r.try_into().unwrap())
                .collect()
        }
        fn state(&mut self) -> (State, Vec<Building>) {
            let mut state = State::from_bytes(self.take(0xe04)).unwrap();
            let buildings = self.buildings();
            state.counters = std::array::from_fn(|_| self.word());
            (state, buildings)
        }
    }
    #[test]
    #[ignore = "requires original pending construction/demolition observations"]
    fn matches_native_transitions() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PENDING_WORLD")).unwrap();
        let mut r = Reader {
            bytes: &bytes,
            at: 0,
        };
        assert_eq!(r.take(8), b"MTPEND01");
        assert_eq!(r.word(), 5);
        for _ in 0..r.word() {
            let name = r.name();
            r.json();
            let metadata: [u32; 5] = std::array::from_fn(|_| r.word());
            let (mut state, mut buildings) = r.state();
            let (expected, expected_buildings) = r.state();
            let mut calls = Vec::new();
            for _ in 0..r.word() {
                let target = r.word();
                let receiver = r.word();
                r.word();
                let args = (0..r.word()).map(|_| r.word()).collect::<Vec<_>>();
                calls.push((target, receiver, args));
            }
            r.word();
            assert_eq!(r.word(), 0);
            let mut actual = Vec::new();
            for _ in 0..metadata[4] {
                state
                    .advance(
                        &mut buildings,
                        &mut Controls {
                            network_enabled: metadata[0] != 0,
                            network_override: metadata[1] != 0,
                            effect_model_reference: 0,
                        },
                        |event, _, _, _| {
                            match event {
                                Event::Progress { group, building } => actual.push((
                                    0x4b0750,
                                    vec![
                                        [0x8b0260, 0x8af418, 0x8b0240, 0x8b0280][group],
                                        [0x8b03bc, 0x8af410, 0x8b023c, 0x8b03c0][group],
                                        2,
                                        0x591318 + building as u32 * 916 + 0x58,
                                    ],
                                )),
                                Event::Complete {
                                    owner: _,
                                    payload,
                                    building_id,
                                } => actual.push((0x48ad80, vec![payload, building_id, 1])),
                                Event::Remove {
                                    category,
                                    building_id,
                                } => {
                                    let expected = &calls[actual.len()];
                                    assert_eq!(expected.1, 0x617628 + category * 380);
                                    actual.push((0x445d10, vec![building_id]));
                                }
                                Event::ModelHeight { .. } => return Ok(metadata[2]),
                                Event::CreateEffect { arguments } => {
                                    actual.push((0x4eb520, arguments.to_vec()));
                                    return Ok(metadata[3]);
                                }
                            }
                            Ok(0)
                        },
                    )
                    .unwrap();
            }
            assert_eq!(
                actual,
                calls.iter().map(|c| (c.0, c.2.clone())).collect::<Vec<_>>(),
                "{name}: callbacks"
            );
            assert_eq!(state, expected, "{name}: pool");
            for (i, (a, e)) in buildings.iter().zip(&expected_buildings).enumerate() {
                assert_eq!(a, e, "{name}: building{i}");
            }
        }
        assert_eq!(r.at, bytes.len());
    }
    #[test]
    #[ignore = "requires original pending-world producer observations"]
    fn matches_native_producers() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PENDING_PRODUCERS")).unwrap();
        let mut r = Reader {
            bytes: &bytes,
            at: 0,
        };
        assert_eq!(r.take(8), b"MTPROD01");
        assert_eq!(r.word(), 2);
        for _ in 0..r.word() {
            let name = r.name();
            r.json();
            let meta: [u32; 6] = std::array::from_fn(|_| r.word());
            let mut state = State::from_bytes(r.take(0xe04)).unwrap();
            let expected = State::from_bytes(r.take(0xe04)).unwrap();
            let mut buildings = r.buildings();
            let expected_buildings = r.buildings();
            r.word();
            let eax = r.word();
            let fault = r.word();
            let result = if meta[0] == 1 {
                state
                    .construct(meta[1], &mut buildings[meta[2] as usize], meta[4], meta[5])
                    .map(|_| true)
            } else {
                state.demolish(meta[1], &mut buildings[meta[2] as usize])
            };
            if fault != 0 {
                assert!(result.is_err(), "{name}: full original pool faults");
            } else {
                assert_eq!(result.unwrap(), eax as u8 != 0, "{name}: result");
            }
            assert_eq!(state, expected, "{name}: pool");
            for (i, (a, e)) in buildings.iter().zip(&expected_buildings).enumerate() {
                assert_eq!(a, e, "{name}: building{i}");
            }
        }
        assert_eq!(r.at, bytes.len());
    }
}
