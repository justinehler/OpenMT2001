//! Original Commodity property/membership bindings over authoritative raw rows.
use crate::lua4::{Runtime, Value};

pub struct State {
    pub rows: [[u8; 0x1c]; 32],
    /// Native address order: snack, main, entertainment, dining, stay-over.
    pub memberships: [u32; 5],
}

impl State {
    /// Image defaults only; reset/load policies provide the live membership
    /// globals explicitly. The supplied commodity script then changes these rows.
    pub fn from_v14(image: &crate::pe::PeImage<'_>, memberships: [u32; 5]) -> Result<Self, String> {
        let source = image.at(0x53be98, 32 * 0x1c)?;
        Ok(Self {
            rows: std::array::from_fn(|i| source[i * 0x1c..(i + 1) * 0x1c].try_into().unwrap()),
            memberships,
        })
    }

    pub fn resident_inputs(&self) -> [crate::resident::Commodity; 32] {
        self.rows.map(|row| crate::resident::Commodity {
            active: row[0x10] != 0,
            value: f32::from_le_bytes(row[0x14..0x18].try_into().unwrap()),
            markup: f32::from_le_bytes(row[0x18..0x1c].try_into().unwrap()),
        })
    }
}

pub struct Bindings {
    first: u32,
}
impl Bindings {
    ///4da440:32 entries with bool active, f32 value/markup, and Membership.
    pub fn register(
        runtime: &mut Runtime,
        image: &crate::pe::PeImage<'_>,
        first: u32,
    ) -> Result<Self, String> {
        first
            .checked_add(64)
            .ok_or("commodity callback ID overflow")?;
        for (name, index) in crate::resident::commodity_names(image)? {
            runtime.set_number(name, f64::from(index))?;
        }
        for (name, index) in [
            ("MAINSHOPPING", 0),
            ("DININGOUTSHOPPING", 1),
            ("SNACKSHOPPING", 2),
            ("ENTERTAINMENTSHOPPING", 3),
            ("STAYOVERSHOPPING", 4),
        ] {
            runtime.set_number(name, f64::from(index))?;
        }
        runtime.execute("Commodity table", b"Commodity={}", &mut |_, _| {
            Err("unexpected registration callback".into())
        })?;
        for index in 0..32 {
            runtime.register_number_properties("openmt_commodity", first + index * 2)?;
            runtime.register_indexed_method("openmt_commodity", "Membership", first + 64, index)?;
            runtime.execute(
                "Commodity entry",
                format!("Commodity[{index}]=openmt_commodity").as_bytes(),
                &mut |_, _| Err("unexpected registration callback".into()),
            )?;
        }
        runtime.set_number("TRUE", 1.)?;
        runtime.set_number("true", 1.)?;
        runtime.execute(
            "commodity constants",
            b"FALSE=nil; false=nil; openmt_commodity=nil",
            &mut |_, _| Err("unexpected registration callback".into()),
        )?;
        Ok(Self { first })
    }

    pub fn contains(&self, id: u32) -> bool {
        id.checked_sub(self.first).is_some_and(|id| id <= 64)
    }

    pub fn handle(
        &self,
        id: u32,
        args: &[Value],
        state: &mut State,
    ) -> Result<Option<f64>, String> {
        let id = id
            .checked_sub(self.first)
            .filter(|id| *id <= 64)
            .ok_or("unknown commodity callback")?;
        if id == 64 {
            let Some(Value::Number(index)) = args.first() else {
                return Err("membership requires bound index".into());
            };
            let index = crate::economy::ftol_low_word(*index) as u32;
            let row = state
                .rows
                .get(index as usize)
                .ok_or("commodity receiver leaves table")?;
            let commodity_id = u32::from_le_bytes(row[..4].try_into().unwrap());
            let category = match args.get(1) {
                Some(Value::Number(v)) => crate::economy::ftol_low_word(*v) as u32,
                _ => 0,
            };
            if let Some(&slot) = [1, 3, 0, 2, 4].get(category as usize) {
                state.memberships[slot] |= 1_u32.wrapping_shl(commodity_id);
            }
            return Ok(None);
        }
        let Some(Value::Bytes(key)) = args.first() else {
            return Err("commodity key must be a string".into());
        };
        let offset = match key.as_slice() {
            b"active" => 0x10,
            b"value" => 0x14,
            b"markup" => 0x18,
            _ => return Err("unrecovered commodity property".into()),
        };
        let row = &mut state.rows[(id / 2) as usize];
        if id % 2 == 1 {
            let Some(Value::Number(value)) = args.get(1) else {
                return Err("commodity setter requires numeric bridge".into());
            };
            if offset == 0x10 {
                row[offset] = u8::from(*value != 0. && !value.is_nan());
            } else {
                row[offset..offset + 4].copy_from_slice(&(*value as f32).to_le_bytes());
            }
            Ok(None)
        } else {
            Ok(Some(if offset == 0x10 {
                f64::from(row[offset])
            } else {
                f64::from(f32::from_le_bytes(
                    row[offset..offset + 4].try_into().unwrap(),
                ))
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original commodity script, native membership fixture, and supplied save"]
    fn executes_complete_commodity_settings_without_saved_inputs() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let source =
            std::fs::read(std::env::var("OPENMT2001_COMMODITY_SETTINGS").unwrap()).unwrap();
        let native = std::fs::read(std::env::var("OPENMT2001_NATIVE_MEMBERSHIP").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTCMEM01");
        for at in [16, 56] {
            let mut state =
                State::from_v14(&image, std::array::from_fn(|i| word(at + i * 4))).unwrap();
            let before = state.rows;
            let mut runtime = Runtime::new().unwrap();
            let bindings = Bindings::register(&mut runtime, &image, 100).unwrap();
            runtime
                .execute("commoditysettings.lua", &source, &mut |id, args| {
                    bindings.handle(id, args, &mut state)
                })
                .unwrap();
            assert_eq!(
                state.memberships,
                std::array::from_fn(|i| word(at + 20 + i * 4))
            );
            for (old, new) in before.iter().zip(&state.rows) {
                assert_eq!(&old[..0x10], &new[..0x10]);
                assert_eq!(&old[0x11..0x14], &new[0x11..0x14]);
            }
            assert_eq!(state.rows.iter().filter(|r| r[0x10] != 0).count(), 22);
            let saved = std::fs::read(std::env::var("OPENMT2001_SAVE_ONE").unwrap()).unwrap();
            let save = crate::save::SaveFile::parse(&saved).unwrap();
            for (fresh, saved) in state
                .resident_inputs()
                .iter()
                .zip(save.economy().unwrap().resident_commodities())
            {
                assert_eq!(
                    (fresh.active, fresh.value.to_bits(), fresh.markup.to_bits()),
                    (saved.active, saved.value.to_bits(), saved.markup.to_bits())
                );
            }
            let before = state.memberships;
            runtime
                .execute(
                    "invalid membership",
                    b"Commodity[MEAT]:Membership(-1); Commodity[MEAT]:Membership(5)",
                    &mut |id, args| bindings.handle(id, args, &mut state),
                )
                .unwrap();
            assert_eq!(state.memberships, before);
        }
    }
}
