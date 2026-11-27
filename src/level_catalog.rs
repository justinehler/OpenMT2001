//! Original level registrations48d320/48d710 and retained profile progression.
//! List handles are one-based indices, rather than original process addresses.
use crate::player_manager::{put, word};

pub const RECORD_SIZE: usize = 0x884;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub bytes: [u8; RECORD_SIZE],
}
impl Level {
    ///4dacb0 reads this selected-record binary32, independent of86ce64.
    pub fn objective_target(&self) -> f32 {
        f32::from_bits(word(&self.bytes, 0x870))
    }
    ///4a4815..4a4831 writes the setup modifier before roster handoff.
    pub fn set_objective_target(&mut self, target: f32) {
        put(&mut self.bytes, 0x870, target.to_bits());
    }
    pub fn id(&self) -> u32 {
        word(&self.bytes, 0x44c)
    }
    pub fn directory(&self) -> &[u8] {
        let bytes = &self.bytes[0x48..0x44c];
        &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]
    }
    pub fn tutorial(&self) -> bool {
        self.bytes[0x85c] != 0
    }
    pub fn single_player(&self) -> bool {
        self.bytes[1] != 0
    }
    pub fn unlocked(&self) -> bool {
        word(&self.bytes, 0x86c) != 0
    }
    pub fn completed(&self) -> [bool; 3] {
        std::array::from_fn(|i| self.bytes[0x874 + i] != 0)
    }
    pub fn attempts(&self) -> u32 {
        word(&self.bytes, 0x868)
    }
    pub fn labels(&self) -> Labels {
        if self.bytes[0x87c] != 0 {
            Labels::Localized {
                title: word(&self.bytes, 0x44),
                description: word(&self.bytes, 0x854),
            }
        } else {
            let text = |bytes: &[u8]| {
                bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())].to_vec()
            };
            Labels::Literal {
                title: text(&self.bytes[3..0x44]),
                description: text(&self.bytes[0x454..0x854]),
            }
        }
    }
}
#[derive(Clone, Debug)]
pub enum Labels {
    Localized {
        title: u32,
        description: u32,
    },
    Literal {
        title: Vec<u8>,
        description: Vec<u8>,
    },
}
pub struct Registration {
    pub id: i32,
    pub directory: Vec<u8>,
    pub labels: Labels,
    ///48d320 sums unsigned bytes of the directory's Lua files, wrapping32-bit.
    pub checksum: i32,
}
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub levels: Vec<Level>,
    pub selected: Option<usize>,
    pub difficulty: u32,
}
fn string_into(to: &mut [u8], from: &[u8]) -> Result<(), String> {
    let from = &from[..from.iter().position(|&b| b == 0).unwrap_or(from.len())];
    if from.len() >= to.len() {
        return Err("level registration string exceeds original field".into());
    }
    to[..from.len()].copy_from_slice(from);
    to[from.len()] = 0;
    Ok(())
}
impl Catalog {
    ///48d320 checks the developer gate and hub existence before allocation.
    ///Duplicate IDs remain separate records; native lookup selects the first.
    pub fn register(
        &mut self,
        input: Registration,
        development: bool,
        hub_exists: bool,
    ) -> Result<Option<usize>, String> {
        let mode = input.id / 100000;
        if (!development && mode >= 7) || !hub_exists {
            return Ok(None);
        }
        let mut bytes = [0; RECORD_SIZE];
        bytes[0] = 1;
        bytes[1] = u8::from(mode != 3 && mode != 9);
        bytes[0x85c] = u8::from(mode == 1 || mode == 7);
        bytes[2] = u8::from(mode == 5);
        bytes[0x85d] = u8::from(mode >= 7);
        put(&mut bytes, 0x86c, u32::from(mode == 5));
        put(&mut bytes, 0x44c, input.id as u32);
        put(&mut bytes, 0x878, input.checksum as u32);
        string_into(&mut bytes[0x48..0x44c], &input.directory)?;
        match input.labels {
            Labels::Localized { title, description } => {
                put(&mut bytes, 0x44, title);
                put(&mut bytes, 0x854, description);
                bytes[0x87c] = 1;
            }
            Labels::Literal { title, description } => {
                string_into(&mut bytes[3..0x44], &title)?;
                string_into(&mut bytes[0x454..0x854], &description)?;
            }
        }
        let index = self.levels.len();
        if let Some(last) = self.levels.last_mut() {
            put(&mut last.bytes, 0x880, index as u32 + 1);
        } else {
            self.selected = Some(0);
        }
        self.levels.push(Level { bytes });
        Ok(Some(index))
    }
    pub fn find(&self, id: u32) -> Option<usize> {
        self.levels.iter().position(|l| l.id() == id)
    }
    pub fn find_directory(&self, name: &[u8]) -> Option<usize> {
        self.levels
            .iter()
            .position(|l| l.directory().eq_ignore_ascii_case(name))
    }
    pub fn selected_level(&self) -> Option<&Level> {
        self.selected.and_then(|i| self.levels.get(i))
    }
    ///48d740 updates the cursor even when selection fails. The optional
    ///multiplayer script execution is a caller boundary, not progression.
    pub fn select(&mut self, id: u32, network: bool) -> bool {
        self.selected = if id == u32::MAX {
            None
        } else {
            self.levels
                .iter()
                .position(|l| l.single_player() != network && l.id() == id)
        };
        self.selected.is_some()
    }
    ///48dab0, before a profile's SetLevelInfo records are applied.
    pub fn reset_progress(&mut self) {
        for level in &mut self.levels {
            for at in [0x864, 0x860, 0x868, 0x86c] {
                put(&mut level.bytes, at, 0);
            }
            level.bytes[0x874..0x877].fill(0);
            if level.bytes[0x85d] != 0 {
                put(&mut level.bytes, 0x86c, 1);
            }
        }
    }
    ///48db10 unlocks the first non-tutorial record, if its value is zero.
    pub fn unlock_first(&mut self) {
        if let Some(level) = self.levels.iter_mut().find(|l| !l.tutorial())
            && word(&level.bytes, 0x86c) == 0
        {
            put(&mut level.bytes, 0x86c, 1);
        }
    }
    ///48db90 silently does nothing when the identifier is not registered.
    pub fn unlock(&mut self, id: u32) {
        if let Some(i) = self.find(id) {
            put(&mut self.levels[i].bytes, 0x86c, 1);
        }
    }
    ///48db50: selected record followed by the next non-tutorial record.
    pub fn unlock_next(&mut self) -> Result<(), String> {
        let i = self
            .selected
            .filter(|&i| i < self.levels.len())
            .ok_or("level progression selected record unresolved")?;
        put(&mut self.levels[i].bytes, 0x86c, 1);
        if let Some(level) = self.levels[i + 1..].iter_mut().find(|l| !l.tutorial()) {
            put(&mut level.bytes, 0x86c, 1);
        }
        Ok(())
    }
    ///48dbd0 has a null-selection guard; the difficulty indexes three bytes.
    pub fn mark_completed(&mut self) -> Result<(), String> {
        if let Some(i) = self.selected {
            let d = usize::try_from(self.difficulty)
                .ok()
                .filter(|&d| d < 3)
                .ok_or("level completion difficulty outside three")?;
            let level = self
                .levels
                .get_mut(i)
                .ok_or("level completion selected record unresolved")?;
            level.bytes[0x874 + d] = 1;
        }
        Ok(())
    }
    ///4c1950: argument2 is obsolete; listed levels are always unlocked.
    pub fn set_level_info(&mut self, id: u32, attempts: u32, completed: [i32; 3]) {
        if let Some(i) = self.find(id) {
            let level = &mut self.levels[i];
            put(&mut level.bytes, 0x86c, 1);
            put(&mut level.bytes, 0x868, attempts);
            for (d, v) in completed.into_iter().enumerate() {
                level.bytes[0x874 + d] = u8::from(v != 0);
            }
        }
    }
    ///4ba2a4..4ba311, after gameplay teardown and before profile persistence.
    pub fn finish(&mut self, completed: bool, won: bool) -> Result<(), String> {
        if !completed {
            return Ok(());
        }
        let i = self
            .selected
            .filter(|&i| i < self.levels.len())
            .ok_or("finished level unresolved")?;
        if self.levels[i].tutorial() {
            return self.mark_completed();
        }
        let attempts = self.levels[i].attempts().wrapping_add(1);
        put(&mut self.levels[i].bytes, 0x868, attempts);
        if won {
            self.mark_completed()?;
            self.unlock_next()?;
        }
        Ok(())
    }
    ///4c244f: only exactly-one unlocks and all tutorial rows are serialized.
    pub fn profile_records(&self) -> impl Iterator<Item = (u32, u32, [bool; 3])> + '_ {
        self.levels
            .iter()
            .filter(|l| word(&l.bytes, 0x86c) == 1 || l.tutorial())
            .map(|l| (l.id(), l.attempts(), l.completed()))
    }
}

#[cfg(feature = "lua4")]
#[path = "level_catalog/script.rs"]
pub mod script;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original level catalog/progression fixture"]
    fn matches_native_catalog() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_LEVEL_CATALOG").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(&data[..8], b"MTLVL002");
        assert_eq!(word(8), 0x37f);
        let mut at = 16;
        let blob = |at: &mut usize| {
            let n = word(*at) as usize;
            *at += 4;
            let b = &data[*at..*at + n];
            *at += n;
            b
        };
        let decode = |b: &[u8]| {
            let w = |i| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
            Catalog {
                levels: b[12..]
                    .chunks_exact(RECORD_SIZE)
                    .map(|b| Level {
                        bytes: b.try_into().unwrap(),
                    })
                    .collect(),
                selected: w(4).checked_sub(1).map(|i| i as usize),
                difficulty: w(8),
            }
        };
        for case in 0..word(12) {
            let args: [u32; 12] = std::array::from_fn(|i| word(at + i * 4));
            at += 48;
            let token = blob(&mut at);
            let title = blob(&mut at);
            let description = blob(&mut at);
            let mut actual = decode(blob(&mut at));
            let expected = decode(blob(&mut at));
            let completed = word(at) != 0;
            let won = word(at + 4) != 0;
            at += 8;
            let serialized = blob(&mut at);
            match args[0] {
                0 => {
                    actual
                        .register(
                            Registration {
                                id: args[1] as i32,
                                directory: token.to_vec(),
                                labels: if args[4] != 0 {
                                    Labels::Localized {
                                        title: args[6],
                                        description: args[7],
                                    }
                                } else {
                                    Labels::Literal {
                                        title: title.to_vec(),
                                        description: description.to_vec(),
                                    }
                                },
                                checksum: args[5] as i32,
                            },
                            args[2] != 0,
                            args[3] != 0,
                        )
                        .unwrap();
                }
                1 => actual.reset_progress(),
                2 => actual.unlock_first(),
                3 => actual.unlock(args[1]),
                4 => actual.unlock_next().unwrap(),
                5 => actual.mark_completed().unwrap(),
                6 => actual.set_level_info(
                    args[1],
                    args[8],
                    [args[9] as i32, args[10] as i32, args[11] as i32],
                ),
                7 => actual.finish(completed, won).unwrap(),
                8 => {
                    actual.select(args[1], false);
                }
                _ => panic!("unknown catalog operation"),
            }
            assert_eq!(actual.selected, expected.selected, "case{case} selection");
            assert_eq!(actual.difficulty, expected.difficulty);
            assert_eq!(
                actual.levels.len(),
                expected.levels.len(),
                "case{case} count"
            );
            let mut text = String::new();
            for (id, attempts, done) in actual.profile_records() {
                text.push_str(&format!(
                    "SetLevelInfo({}, DEPRECATED, {}, {}, {}, {})\n",
                    id as i32,
                    attempts as i32,
                    u8::from(done[0]),
                    u8::from(done[1]),
                    u8::from(done[2])
                ));
            }
            assert_eq!(text.as_bytes(), serialized, "case{case} profile writer");
            for (i, (a, b)) in actual.levels.iter().zip(&expected.levels).enumerate() {
                for (at, (a, b)) in a.bytes.iter().zip(b.bytes).enumerate() {
                    assert_eq!(*a, b, "case{case} level{i}+{at:x}");
                }
            }
        }
        assert_eq!(at, data.len());
    }
}
