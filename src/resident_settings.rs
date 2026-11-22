//! Live DEFAULT resident preference and welfare bindings.
use crate::lua4::{Runtime, Value};

pub struct State<'a> {
    pub preferences: &'a mut crate::resident::Preferences,
    pub groups: &'a mut crate::resident::PreferenceGroups,
    pub welfare: &'a mut crate::resident::Welfare,
}
pub struct Bindings {
    first: u32,
}
impl Bindings {
    /// Requires the existing InitialValue and People tables. Fixed numeric
    /// conversions match4daac0,4dab10 and4dcf40 before their storage helpers.
    pub fn register(vm: &mut Runtime, first: u32) -> Result<Self, String> {
        first
            .checked_add(2)
            .ok_or("resident settings callback overflow")?;
        vm.register_number_method("InitialValue", "AddPreference", first, 3)?;
        vm.register_number_method("InitialValue", "AssociatePreference", first + 1, 2)?;
        vm.register_number_method("People", "Welfare", first + 2, 3)?;
        for (age, name) in ["CHILD", "TEENAGER", "ADULT", "PENSIONER", "TOURIST"]
            .into_iter()
            .enumerate()
        {
            vm.set_number(&format!("AGE_{name}"), age as f64)?;
            for (class, label) in ["LOW", "MEDIUM", "HIGH"].into_iter().enumerate() {
                vm.set_number(&format!("{label}_CLASS_{name}"), (age * 3 + class) as f64)?;
                vm.set_number(&format!("CLASS_{label}"), class as f64)?;
            }
        }
        Ok(Self { first })
    }
    pub fn contains(&self, id: u32) -> bool {
        (self.first..=self.first + 2).contains(&id)
    }
    pub fn handle(
        &self,
        id: u32,
        args: &[Value],
        state: &mut State<'_>,
    ) -> Result<Option<f64>, String> {
        let number = |index: usize| match args.get(index) {
            Some(Value::Number(value)) => Ok(*value),
            _ => Err("resident setting requires Lua-coerced arguments".to_owned()),
        };
        let integer = |index| number(index).map(crate::economy::ftol_low_word);
        match id.wrapping_sub(self.first) {
            0 => {
                state
                    .preferences
                    .append(integer(0)?, integer(1)?, number(2)? as f32)?;
            }
            1 => {
                state.groups.associate(integer(0)?, integer(1)?)?;
            }
            2 => {
                let age = usize::try_from(integer(0)?).map_err(|_| "negative welfare age")?;
                let class = usize::try_from(integer(1)?).map_err(|_| "negative welfare class")?;
                let cell = state
                    .welfare
                    .amounts
                    .get_mut(class)
                    .and_then(|row| row.get_mut(age))
                    .ok_or("welfare write outside its native fields")?;
                *cell = number(2)? as f32;
            }
            _ => return Err("unknown resident setting callback".into()),
        }
        // Original Lua wrappers discard helper acceptance results.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original preference registration tables"]
    fn matches_original_preference_writes() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_RESIDENT_SETTINGS").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTRPREF1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        let mut preferences = crate::resident::Preferences::empty();
        let mut groups = crate::resident::PreferenceGroups::empty();
        assert_eq!(word(8), 2);
        for phase in 0..2 {
            let operations = word(at);
            at += 4;
            for operation in 0..operations {
                let [kind, row, value, bits, expected] = std::array::from_fn(|i| word(at + i * 4));
                at += 20;
                let accepted = if kind == 0 {
                    preferences
                        .append(row as i32, value as i32, f32::from_bits(bits))
                        .unwrap()
                } else {
                    groups.associate(row as i32, value as i32).unwrap()
                };
                assert_eq!(
                    accepted,
                    expected != 0,
                    "phase {phase} operation {operation}"
                );
            }
            for index in 0..128 {
                let entries = preferences.get(index as i8).unwrap();
                assert_eq!(entries.len(), word(at + 256) as usize);
                for (slot, entry) in entries.iter().enumerate() {
                    assert_eq!(entry.commodity, word(at + slot * 4));
                    assert_eq!(entry.factor.to_bits(), word(at + 128 + slot * 4));
                }
                at += 260;
            }
            for index in 0..15 {
                let row = groups.associations(index).unwrap();
                assert_eq!(row.len(), data[at + 32] as usize);
                assert_eq!(
                    row.iter().map(|v| *v as u8).collect::<Vec<_>>(),
                    data[at..at + row.len()]
                );
                at += 33;
            }
        }
        assert_eq!(at, data.len());
    }
    #[test]
    fn numeric_methods_preserve_live_updates_and_original_return_shape() {
        let mut vm = Runtime::new().unwrap();
        vm.execute(
            "tables",
            b"InitialValue={}; People={}",
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let bindings = Bindings::register(&mut vm, 500).unwrap();
        let mut preferences = crate::resident::Preferences::empty();
        let mut groups = crate::resident::PreferenceGroups::empty();
        let mut welfare = crate::resident::Welfare {
            amounts: [[0.; 5]; 3],
        };
        let mut state = State {
            preferences: &mut preferences,
            groups: &mut groups,
            welfare: &mut welfare,
        };
        vm.execute("live settings",b"result=InitialValue.AddPreference('0.9','31','2.5'); InitialValue.AddPreference(128,2,1); InitialValue.AssociatePreference(LOW_CLASS_ADULT,0); People.Welfare(AGE_ADULT,CLASS_LOW,'63.25'); InitialValue.AddPreference(0,3);", &mut |id,args|bindings.handle(id,args,&mut state)).unwrap();
        let entries = state.preferences.get(0).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!((entries[0].commodity, entries[0].factor), (31, 2.5));
        assert_eq!((entries[1].commodity, entries[1].factor), (3, 0.));
        assert_eq!(state.groups.associations(6).unwrap(), [0]);
        assert_eq!(state.welfare.amount(2, 0), Some(63.25));
        assert_eq!(vm.number("result").unwrap(), None);
    }
}
