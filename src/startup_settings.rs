//! Original DEFAULT scalar settings over caller-owned live configuration.
use crate::lua4::{Runtime, Value};
pub struct State {
    pub num_cars: i32,
    /// Car, van, taxi; six four-hour slots each.
    pub traffic: [[i32; 6]; 3],
    pub itchy_counter: i32,
    pub subway_threshold: i32,
    /// Original93af08 structure, including untouched padding.
    pub tweaks: [u8; 36],
    selected_income: crate::economy::OwnerIncomeSettings,
}
impl State {
    /// Fresh-process zero-filled globals; original4dd1d0 also clears Tweaks.
    pub fn fresh() -> Self {
        Self {
            num_cars: 0,
            traffic: [[0; 6]; 3],
            itchy_counter: 0,
            subway_threshold: 0,
            tweaks: [0; 36],
            selected_income: crate::economy::OwnerIncomeSettings {
                multiplier: 0.,
                daily_freight: 0.,
            },
        }
    }
    /// 0048d130 publishes both selected globals. Later Tweaks writes only
    /// change the configuration; unsupported selectors leave the globals alone.
    pub fn apply_difficulty(&mut self, difficulty: usize) {
        if let (Some(multiplier), Some(daily_freight)) =
            (self.rail_income(difficulty), self.daily_freight(difficulty))
        {
            self.selected_income = crate::economy::OwnerIncomeSettings {
                multiplier,
                daily_freight,
            };
        }
    }

    pub fn selected_income(&self) -> crate::economy::OwnerIncomeSettings {
        self.selected_income
    }

    pub fn years_per_day(&self) -> u32 {
        u32::from_le_bytes(self.tweaks[4..8].try_into().unwrap())
    }
    pub fn lease_length(&self) -> i32 {
        i32::from_le_bytes(self.tweaks[8..12].try_into().unwrap())
    }
    pub fn city_businesses_ownable(&self) -> bool {
        self.tweaks[0] != 0
    }
    pub fn days_off(&self) -> bool {
        self.tweaks[1] != 0
    }
    pub fn rail_income(&self, difficulty: usize) -> Option<f32> {
        (difficulty < 3).then(|| {
            f32::from_le_bytes(
                self.tweaks[12 + difficulty * 4..16 + difficulty * 4]
                    .try_into()
                    .unwrap(),
            )
        })
    }
    pub fn daily_freight(&self, difficulty: usize) -> Option<f32> {
        (difficulty < 3).then(|| {
            f32::from_le_bytes(
                self.tweaks[24 + difficulty * 4..28 + difficulty * 4]
                    .try_into()
                    .unwrap(),
            )
        })
    }
}
pub struct Bindings {
    first: u32,
}
impl Bindings {
    /// Register People before attaching its Create/Welfare functions.
    pub fn register(vm: &mut Runtime, first: u32) -> Result<Self, String> {
        first
            .checked_add(9)
            .ok_or("startup setting callback overflow")?;
        for (i, name) in [
            "People",
            "Tweaks",
            "CarFrequency",
            "VanFrequency",
            "TaxiFrequency",
        ]
        .into_iter()
        .enumerate()
        {
            vm.register_number_properties(name, first + i as u32 * 2)?;
        }
        Ok(Self { first })
    }
    pub fn contains(&self, id: u32) -> bool {
        (self.first..self.first + 10).contains(&id)
    }
    pub fn handle(
        &self,
        id: u32,
        args: &[Value],
        state: &mut State,
    ) -> Result<Option<f64>, String> {
        let local = id
            .checked_sub(self.first)
            .filter(|v| *v < 10)
            .ok_or("unknown startup setting callback")?;
        let Some(Value::Bytes(key)) = args.first() else {
            return Err("startup property key is not a string".into());
        };
        let write = if local % 2 == 1 {
            let Some(Value::Number(value)) = args.get(1) else {
                return Err("startup setter needs numeric bridge".into());
            };
            Some(*value)
        } else {
            None
        };
        let integer = |cell: &mut i32| {
            if let Some(value) = write {
                *cell = crate::economy::ftol_low_word(value);
                None
            } else {
                Some(f64::from(*cell))
            }
        };
        match local / 2 {
            0 => match key.as_slice() {
                b"itchycounter" => Ok(integer(&mut state.itchy_counter)),
                b"subwaythreshold" => Ok(integer(&mut state.subway_threshold)),
                _ => Err("unrecovered People property".into()),
            },
            1 => {
                let (at, kind) = match key.as_slice() {
                    b"citybusinessesownable" => (0, 0),
                    b"daysoff" => (1, 0),
                    b"numyearsin24hours" => (4, 1),
                    b"blockleaselength" => (8, 1),
                    b"railutilincomeEASY" => (12, 2),
                    b"railutilincomeMEDIUM" => (16, 2),
                    b"railutilincomeHARD" => (20, 2),
                    b"dailyfreightEASY" => (24, 2),
                    b"dailyfreightMEDIUM" => (28, 2),
                    b"dailyfreightHARD" => (32, 2),
                    _ => return Err("unrecovered Tweaks property".into()),
                };
                if kind == 0 {
                    if let Some(value) = write {
                        state.tweaks[at] = u8::from(value != 0. && !value.is_nan());
                        return Ok(None);
                    }
                    return Ok(Some(f64::from(state.tweaks[at])));
                }
                if let Some(value) = write {
                    let bits = if kind == 1 {
                        (crate::economy::ftol_low_word(value) as u32).to_le_bytes()
                    } else {
                        (value as f32).to_le_bytes()
                    };
                    state.tweaks[at..at + 4].copy_from_slice(&bits);
                    Ok(None)
                } else {
                    let bytes = state.tweaks[at..at + 4].try_into().unwrap();
                    Ok(Some(if kind == 1 {
                        f64::from(i32::from_le_bytes(bytes))
                    } else {
                        f64::from(f32::from_le_bytes(bytes))
                    }))
                }
            }
            family => {
                let slot = [
                    b"_0to3".as_slice(),
                    b"_4to7",
                    b"_8to11",
                    b"_12to15",
                    b"_16to19",
                    b"_20to23",
                ]
                .iter()
                .position(|name| *name == key)
                .ok_or("unrecovered traffic time slot")?;
                Ok(integer(&mut state.traffic[(family - 2) as usize][slot]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configure(state: &mut State, difficulty: usize, multiplier: f32, freight: f32) {
        state.tweaks[12 + difficulty * 4..16 + difficulty * 4]
            .copy_from_slice(&multiplier.to_bits().to_le_bytes());
        state.tweaks[24 + difficulty * 4..28 + difficulty * 4]
            .copy_from_slice(&freight.to_bits().to_le_bytes());
    }

    #[test]
    fn selected_income_changes_only_when_difficulty_is_applied() {
        let mut settings = State::fresh();
        for difficulty in 0..3 {
            configure(&mut settings, difficulty, 1., 800.);
            settings.apply_difficulty(difficulty);
            configure(&mut settings, difficulty, 2., 1600.);
            let mut utilities = crate::economy::Utilities::new([1, 0, 0, 0]);
            let mut properties = crate::economy::PropertyIncome {
                owners: [1, 0, 0, 0],
                totals: [0.; 4],
            };
            let mut players = [crate::economy::PlayerAccount::from_runtime(&[0; 440]).unwrap()];
            for (reapply, multiplier, freight) in [(false, 1., 800.), (true, 2., 1600.)] {
                if reapply {
                    settings.apply_difficulty(difficulty);
                }
                settings
                    .selected_income()
                    .reset_income(&mut utilities, &mut properties);
                assert_eq!(properties.totals, [freight; 4]);
                utilities.totals = [400., 0., 0., 0.];
                let before = players[0].cash;
                settings
                    .selected_income()
                    .pay_owners(&mut utilities, &mut properties, &mut players)
                    .unwrap();
                assert_eq!(
                    players[0].cash - before,
                    (400. + freight) * 0.25 * multiplier
                );
                assert_eq!(properties.totals, [freight; 4]);
            }
        }
    }

    #[test]
    fn difficulty_selection_preserves_raw_float_bits_and_ignores_invalid_selector() {
        let mut settings = State::fresh();
        configure(&mut settings, 1, f32::from_bits(0x7f80_0001), -0.);
        settings.apply_difficulty(1);
        configure(&mut settings, 1, 2., 1600.);
        settings.apply_difficulty(3);
        let income = settings.selected_income();
        assert_eq!(income.multiplier.to_bits(), 0x7f80_0001);
        assert_eq!(income.daily_freight.to_bits(), (-0.0_f32).to_bits());
    }

    #[test]
    #[ignore = "requires original scalar property observations"]
    fn matches_original_scalar_setters() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_STARTUP_SETTINGS")).unwrap();
        assert_eq!(&data[..8], b"MTSTART1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 31);
        let mut vm = Runtime::new().unwrap();
        let bindings = Bindings::register(&mut vm, 600).unwrap();
        let initial = crate::initial_conditions::Bindings::register(&mut vm, 300).unwrap();
        let mut settings = State::fresh();
        let mut categories = [[0; 0x17c]; 52];
        let mut definitions = crate::initial_conditions::Definitions {
            rows: [[0; 0x58]; 52],
        };
        let mut money = 0.;
        let mut at = 12;
        for _ in 0..word(8) {
            let [table, length, kind, cases] = std::array::from_fn(|i| word(at + i * 4));
            at += 16;
            let key = data[at..at + length as usize].to_vec();
            at += length as usize;
            for case in 0..cases {
                let value = f64::from_le_bytes(data[at..at + 8].try_into().unwrap());
                at += 8;
                let expected = word(at);
                at += 4;
                let mut call = |write| {
                    let args = if write {
                        vec![Value::Bytes(key.clone()), Value::Number(value)]
                    } else {
                        vec![Value::Bytes(key.clone())]
                    };
                    if table == 5 {
                        initial.handle(
                            404 + u32::from(write),
                            &args,
                            &mut crate::initial_conditions::State {
                                num_cars: &mut settings.num_cars,
                                categories: &mut categories,
                                player_money: &mut money,
                                definitions: &mut definitions,
                            },
                        )
                    } else {
                        bindings.handle(600 + table * 2 + u32::from(write), &args, &mut settings)
                    }
                };
                assert_eq!(call(true).unwrap(), None);
                let result = call(false).unwrap().unwrap();
                let actual = match kind {
                    4 => result as i32 as u32,
                    12 => (result as f32).to_bits(),
                    16 => result as u32,
                    _ => unreachable!(),
                };
                assert_eq!(
                    actual,
                    if kind == 16 { expected & 255 } else { expected },
                    "{} case {case}",
                    String::from_utf8_lossy(&key)
                );
            }
        }
        assert_eq!(at, data.len());
    }
}
