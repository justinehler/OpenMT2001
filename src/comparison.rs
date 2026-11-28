//! Player comparison panel state (original 426ba0/427210/425900): the metric
//! controls, the original ranking order, the nine metric values per player and
//! the live trend history. The binary's `comparison` module draws it.
#[derive(Clone, Debug)]
pub struct State {
    pub mode: usize,
    pub enabled: [bool; 9],
    pub open: bool,
}
impl State {
    pub fn new(objective: i32) -> Self {
        let mut result = Self {
            mode: 0,
            enabled: [true; 9],
            open: true,
        };
        result.enabled[2] = false;
        if (0..=6).contains(&objective) {
            result.enabled[7] = false;
            if objective != 1 {
                result.enabled[6] = false;
            }
            result.mode = match objective {
                1 => 6,
                3 => 3,
                4 | 5 => 4,
                6 => 8,
                _ => 0,
            };
        }
        result
    }
    pub fn cycle(&mut self, right: bool) {
        for _ in 0..9 {
            self.mode = (self.mode + if right { 10 } else { 8 }) % 9;
            if self.enabled[self.mode] {
                break;
            }
        }
    }
    pub fn label(&self) -> u32 {
        [1530, 1531, 1532, 1533, 1534, 1535, 1536, 1538, 1386][self.mode]
    }
}
/// Original swap order, including its tie and unordered-comparison behavior.
/// Input rows arrive in reverse manager order from4256a0.
pub fn order(amounts: &[f32]) -> Vec<usize> {
    let mut result: Vec<_> = (0..amounts.len()).collect();
    for i in (0..result.len()).rev() {
        for j in (0..i).rev() {
            if amounts[result[i]] > amounts[result[j]] {
                result.swap(i, j);
            }
        }
    }
    result
}
/// 489fe0 writes60 bytes beginning at86ce60 under tag10000;48a000 restores it.
pub fn saved_objective(save: &crate::save::SaveFile<'_>) -> Result<i32, String> {
    let mut rows = save.sections.iter().filter(|s| s.tag == 0x10000);
    let row = rows
        .next()
        .ok_or("save has no comparison objective descriptor")?;
    if row.payload.len() != 60 || rows.next().is_some() {
        return Err("invalid saved objective descriptor".into());
    }
    Ok(i32::from_le_bytes(row.payload[..4].try_into().unwrap()))
}
#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    pub objective: i32,
    /// Manager index then all nine settled values; reverse manager input order.
    pub rows: Vec<(usize, [f32; 9])>,
}
use crate::player_metrics::Building;
impl Data {
    pub fn from_save(
        save: &crate::save::SaveFile<'_>,
        image: &crate::pe::PeImage<'_>,
        hour: u32,
    ) -> Result<Self, String> {
        let players: Vec<_> = save.players()?.iter().map(|p| *p.bytes).collect();
        let economy = save.economy()?;
        let businesses: Vec<_> = economy.businesses().map(|b| b.bytes.as_slice()).collect();
        let buildings: Vec<_> = save
            .buildings()?
            .into_iter()
            .map(|b| Building {
                category: b.category,
                slot: b.slot,
                bytes: b.bytes,
            })
            .collect();
        let categories = (0..52)
            .map(|i| Ok((save.category_owner(i)?, save.category_prestige(i)?)))
            .collect::<Result<Vec<_>, String>>()?;
        Self::from_records(
            &players,
            &businesses,
            &buildings,
            &categories,
            saved_objective(save)?,
            image,
            hour,
        )
    }
    pub fn from_runtime(
        world: &crate::level_startup::World,
        players: &[[u8; 440]],
        image: &crate::pe::PeImage<'_>,
        building_base: u32,
    ) -> Result<Self, String> {
        Ok(Self {
            objective: world.objective.kind,
            rows: crate::player_metrics::from_runtime(
                &world.city,
                players,
                image,
                world.clock.hour(),
                building_base,
            )?,
        })
    }
    fn from_records(
        players: &[[u8; 440]],
        businesses: &[&[u8]],
        buildings: &[Building<'_>],
        categories: &[(u8, f32)],
        objective: i32,
        image: &crate::pe::PeImage<'_>,
        hour: u32,
    ) -> Result<Self, String> {
        Ok(Self {
            objective,
            rows: crate::player_metrics::comparison(
                players, businesses, buildings, categories, image, hour,
            )?,
        })
    }
    pub fn values(&self, mode: usize) -> Vec<(usize, f32)> {
        self.rows.iter().map(|(i, v)| (*i, v[mode])).collect()
    }
}
/// 427210/event11 changes direction only when the sampled value changes.
/// Equality (including signed zero) and unordered comparisons retain it.
fn trend_up(previous: f32, current: f32, was_up: bool) -> bool {
    if current < previous {
        false
    } else if current > previous {
        true
    } else {
        was_up
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub data: Data,
    pub mode: usize,
    /// Indexed by metric and reverse-manager pair, before rank sorting.
    pub up: [[bool; 6]; 9],
}

/// Retained worker state; rendering and status polling never advance history.
pub struct Runtime {
    pub snapshot: Snapshot,
    previous: [[f32; 6]; 9],
}
impl Runtime {
    pub fn new(data: Data) -> Self {
        let mode = State::new(data.objective).mode;
        let mut runtime = Self {
            snapshot: Snapshot {
                data: data.clone(),
                mode,
                up: [[true; 6]; 9],
            },
            previous: [[0.; 6]; 9],
        };
        // 426ba0 configures labels, then startup43c810 delivers event11.
        runtime.refresh(data);
        runtime
    }
    pub fn refresh(&mut self, data: Data) {
        let mode = self.snapshot.mode;
        for (pair, (_, values)) in data.rows.iter().enumerate() {
            self.snapshot.up[mode][pair] = trend_up(
                self.previous[mode][pair],
                values[mode],
                self.snapshot.up[mode][pair],
            );
            self.previous[mode][pair] = values[mode];
        }
        self.snapshot.data = data;
    }
    pub fn select(&mut self, mode: usize, data: Data) -> Result<(), String> {
        if State::new(data.objective).enabled.get(mode) != Some(&true) {
            return Err("disabled comparison metric".into());
        }
        // Mode controls collect labels without event11: retain this metric's
        // baseline and direction until the next activity refresh.
        self.snapshot.mode = mode;
        self.snapshot.data = data;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direction_retains_equal_and_unordered_samples() {
        for was_up in [false, true] {
            assert!(trend_up(-5., -4., was_up));
            assert!(!trend_up(5., -5., was_up));
            for (previous, current) in [(0., -0.), (42., 42.), (f32::NAN, 1.), (1., f32::NAN)] {
                assert_eq!(trend_up(previous, current, was_up), was_up);
            }
        }
    }

    #[test]
    fn live_updates_retain_each_metric_history_across_switches_and_ranking() {
        let data = |cash, sales, profit| Data {
            objective: 0,
            rows: vec![
                (2, [cash, 0., 0., sales, 300., profit, 0., 0., 0.]),
                (0, [200.; 9]),
            ],
        };
        let mut live = Runtime::new(data(100., 0., -50.));
        assert!(live.snapshot.up.iter().flatten().all(|&up| up));
        live.refresh(data(80., 1., -60.));
        assert!(!live.snapshot.up[0][0]);
        live.refresh(data(80., 2., -70.));
        assert!(!live.snapshot.up[0][0], "equal cash retains down");
        live.select(3, data(80., 2., -70.)).unwrap();
        assert!(live.snapshot.up[3][0], "new metric starts up");
        live.refresh(data(80., 3., -70.));
        assert!(live.snapshot.up[3][0], "sales increased from zero baseline");
        live.refresh(data(80., 4., -70.));
        assert!(live.snapshot.up[3][0]);
        live.select(5, data(80., 4., -70.)).unwrap();
        assert!(live.snapshot.up[5][0], "selection does not advance history");
        live.refresh(data(80., 4., -70.));
        assert!(
            !live.snapshot.up[5][0],
            "negative profit below initial zero"
        );
        live.refresh(data(80., 4., -60.));
        assert!(live.snapshot.up[5][0], "improving negative profit is up");
        live.select(0, data(250., 4., -60.)).unwrap();
        assert!(
            !live.snapshot.up[0][0],
            "cash retains last direction on return"
        );
        live.refresh(data(250., 4., -60.));
        assert!(live.snapshot.up[0][0]);
        live.refresh(data(240., 4., -60.));
        assert!(!live.snapshot.up[0][0]);
        assert_eq!(order(&[240., 200.]), vec![0, 1]);
        assert!(
            live.snapshot.up[0][1],
            "direction follows manager pair, not rank"
        );
        assert!(live.select(2, data(240., 4., -60.)).is_err());
    }

    #[test]
    #[ignore = "requires original saved comparison metrics"]
    fn matches_native_saved_comparison() {
        let b = std::fs::read(crate::fixtures::path("NATIVE_PLAYER_COMPARISON")).unwrap();
        assert_eq!(&b[..8], b"MTCOMP01");
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 3);
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut at = 12;
        for name in [
            "OPENMT2001_SAVE_ONE",
            "OPENMT2001_SAVE_TWO",
            "OPENMT2001_COMPARISON_SYNTHETIC",
        ] {
            let bytes = std::fs::read(crate::fixtures::path(name)).unwrap();
            let save = crate::save::SaveFile::parse(&bytes).unwrap();
            at += 32;
            assert_eq!(word(at) as usize, save.players().unwrap().len());
            at += 4;
            for _ in 0..36 {
                let hour = word(at);
                let mode = word(at + 4) as usize;
                let count = word(at + 8) as usize;
                at += 12;
                let expected: Vec<_> = (0..count)
                    .map(|i| (word(at + i * 8) as usize, word(at + i * 8 + 4)))
                    .collect();
                at += count * 8;
                let data = Data::from_save(&save, &image, hour).unwrap();
                let actual: Vec<_> = data
                    .values(mode)
                    .iter()
                    .map(|(i, v)| (*i, v.to_bits()))
                    .collect();
                assert_eq!(actual, expected, "{name} hour{hour} mode{mode}");
            }
        }
        assert_eq!(at, b.len());
    }

    #[test]
    #[ignore = "requires original comparison control observations"]
    fn matches_native_comparison_controls() {
        let b = std::fs::read(crate::fixtures::path("NATIVE_PLAYER_COMPARISON_CONTROLS")).unwrap();
        assert_eq!(&b[..8], b"MTCMCTL1");
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for _ in 0..word(8) {
            let state = State::new(word(at) as i32);
            assert_eq!(state.mode, word(at + 4) as usize);
            assert_eq!(state.enabled.map(u8::from), b[at + 8..at + 17]);
            at += 17;
            for initial in 0..9 {
                for right in [false, true] {
                    let mut s = state.clone();
                    s.mode = initial;
                    s.cycle(right);
                    assert_eq!(s.mode, word(at) as usize);
                    at += 4;
                }
            }
        }
        let cases = word(at);
        at += 4;
        for _ in 0..cases {
            let n = word(at) as usize;
            at += 4;
            let values: Vec<_> = (0..n).map(|i| f32::from_bits(word(at + i * 4))).collect();
            at += n * 4;
            let expected: Vec<_> = (0..n).map(|i| word(at + i * 4) as usize).collect();
            at += n * 4;
            assert_eq!(order(&values), expected);
        }
        assert_eq!(at, b.len());
    }
}
