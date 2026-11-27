//! Recovered Block and InitialValue properties used before roster construction.
use crate::lua4::{Runtime, Value};

/// Mutable descriptor table005385a0. Block settings target these rows separately
/// from the live categories; resetting live prestige before loading defaults
/// must not be replaced with copying the subsequently edited descriptor value.
pub struct Definitions {
    pub rows: [[u8; 0x58]; 52],
}
impl Definitions {
    pub fn from_v14(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let bytes = image.at(0x5385a0, 52 * 0x58)?;
        Ok(Self {
            rows: std::array::from_fn(|i| bytes[i * 0x58..(i + 1) * 0x58].try_into().unwrap()),
        })
    }
    pub fn reset_values(&self) -> [u32; 52] {
        self.rows
            .map(|row| u32::from_le_bytes(row[0x54..0x58].try_into().unwrap()))
    }
}

/// References to live world storage, not a second copy of category state.
pub struct State<'a> {
    pub num_cars: &'a mut i32,
    pub categories: &'a mut [[u8; 0x17c]; 52],
    pub player_money: &'a mut f32,
    pub definitions: &'a mut Definitions,
}

pub struct Bindings {
    first: u32,
}

impl Bindings {
    /// Read the52 literal name/value registrations in original4d81f0.
    /// These names differ from descriptor names for duplicate park/hall slots
    /// and preserve the original PENSYLVANIA_RAILROAD spelling.
    pub fn register_block_constants(
        runtime: &mut Runtime,
        image: &crate::pe::PeImage<'_>,
    ) -> Result<(), String> {
        // Literal registrations004d81f3..004d824f.
        for (index, name) in [
            "RETAIL",
            "RESIDENTIAL",
            "LANDSCAPE",
            "RETAIL_EMPTY",
            "RESIDENTIAL_EMPTY",
        ]
        .into_iter()
        .enumerate()
        {
            runtime.set_number(name, index as f64)?;
        }
        for index in 0..52_u32 {
            let address = if index == 0 {
                0x4d8254
            } else {
                0x4d8265 + (index - 1) * 20
            };
            let size = if index == 0 { 17 } else { 20 };
            let code = image.at(address, size)?;
            let word = |at| u32::from_le_bytes(code[at..at + 4].try_into().unwrap());
            let mov = if index == 0 { 7 } else { 10 };
            if code[0] != 0x68
                || code[mov] != 0xb9
                || word(mov + 1) != 0x93aab0
                || code[mov + 5] != 0xe8
                || (address + size as u32).wrapping_add(word(mov + 6)) != 0x4d64a0
                || (index == 0 && code[5..7] != [0x6a, 0])
                || (index != 0 && (code[5] != 0x68 || word(6) != (index as f32).to_bits()))
            {
                return Err("unsupported original block constant registration".into());
            }
            let name = image.at(word(1), 64)?;
            let end = name
                .iter()
                .position(|b| *b == 0)
                .ok_or("unterminated block constant name")?;
            runtime.set_number(
                std::str::from_utf8(&name[..end]).map_err(|_| "invalid block constant name")?,
                f64::from(index),
            )?;
        }
        Ok(())
    }
    /// Register52 Block tables and InitialValue playermoney/numcars. Other original
    /// class properties/methods are intentionally unresolved and return errors.
    pub fn register(runtime: &mut Runtime, first: u32) -> Result<Self, String> {
        first
            .checked_add(106)
            .ok_or("initial-condition callback IDs overflow")?;
        runtime.execute("Block table", b"Block={}", &mut |_, _| {
            Err("unexpected initialization callback".into())
        })?;
        for index in 0..52 {
            runtime.register_number_properties("openmt_block", first + index * 2)?;
            runtime.register_indexed_method("openmt_block", "Distribution", first + 106, index)?;
            runtime.execute(
                "Block entry",
                format!("Block[{index}]=openmt_block").as_bytes(),
                &mut |_, _| Err("unexpected initialization callback".into()),
            )?;
        }
        runtime.register_number_properties("InitialValue", first + 104)?;
        //4d5874..4d58d4: false is nil, not the truthy Lua4 number zero.
        runtime.set_number("TRUE", 1.)?;
        runtime.set_number("true", 1.)?;
        runtime.execute(
            "boolean constants",
            b"FALSE=nil; false=nil; openmt_block=nil",
            &mut |_, _| Err("unexpected initialization callback".into()),
        )?;
        Ok(Self { first })
    }

    pub fn contains(&self, id: u32) -> bool {
        id.checked_sub(self.first).is_some_and(|id| id < 107)
    }

    pub fn handle(
        &self,
        id: u32,
        args: &[Value],
        state: &mut State<'_>,
    ) -> Result<Option<f64>, String> {
        let id = id
            .checked_sub(self.first)
            .filter(|id| *id < 107)
            .ok_or("unknown initial-condition callback")?;
        if id == 106 {
            let Some(Value::Number(receiver)) = args.first() else {
                return Err("missing Block receiver".into());
            };
            if args.len() < 3 {
                return Err("Distribution needs index and value".into());
            }
            let [Value::Number(index), Value::Number(value)] = &args[args.len() - 2..] else {
                return Err("Distribution requires numeric bridge".into());
            };
            let index = crate::economy::ftol_low_word(*index);
            if !(0..5).contains(&index) {
                return Err("Distribution index leaves the five-slot table".into());
            }
            let row = state
                .definitions
                .rows
                .get_mut(*receiver as usize)
                .ok_or("Block receiver leaves descriptor table")?;
            let at = 0x2c + index as usize * 4;
            row[at..at + 4].copy_from_slice(&(*value as f32).to_le_bytes());
            return Ok(None);
        }
        let Some(Value::Bytes(key)) = args.first() else {
            return Err("property key must be a string".into());
        };
        let assigned = if id % 2 == 1 {
            let Some(Value::Number(number)) = args.get(1) else {
                return Err("property setter requires numeric bridge".into());
            };
            Some(*number)
        } else {
            None
        };
        if id >= 104 {
            if key == b"numcars" {
                if let Some(value) = assigned {
                    *state.num_cars = crate::economy::ftol_low_word(value);
                    return Ok(None);
                }
                return Ok(Some(f64::from(*state.num_cars)));
            }
            if key != b"playermoney" {
                return Err("unrecovered InitialValue property".into());
            }
            if let Some(value) = assigned {
                *state.player_money = value as f32;
                return Ok(None);
            }
            return Ok(Some(f64::from(*state.player_money)));
        }
        if let Some((offset, integer)) = match key.as_slice() {
            b"value" => Some((0x28, true)),
            b"looseness" => Some((0x44, false)),
            b"prestige" => Some((0x54, false)),
            _ => None,
        } {
            let row = &mut state.definitions.rows[(id / 2) as usize];
            if let Some(value) = assigned {
                let bytes = if integer {
                    (crate::economy::ftol_low_word(value) as u32).to_le_bytes()
                } else {
                    (value as f32).to_le_bytes()
                };
                row[offset..offset + 4].copy_from_slice(&bytes);
                return Ok(None);
            }
            let bytes = row[offset..offset + 4].try_into().unwrap();
            return Ok(Some(if integer {
                f64::from(i32::from_le_bytes(bytes))
            } else {
                f64::from(f32::from_le_bytes(bytes))
            }));
        }
        let record = &mut state.categories[(id / 2) as usize];
        let (offset, boolean) = match key.as_slice() {
            b"ownable" => (0x74, true),
            b"buildingrights" => (0x78, false),
            b"owner" => (5, false),
            _ => return Err("unrecovered Block property".into()),
        };
        if let Some(value) = assigned {
            record[offset] = if boolean {
                u8::from(value != 0. && !value.is_nan())
            } else {
                crate::economy::ftol_low_word(value) as u8
            };
            Ok(None)
        } else {
            //4d6c25 numeric bytes read signed;4d6c6e booleans read unsigned
            //numbers, even zero (which remains truthy in Lua4).
            Ok(Some(if boolean {
                f64::from(record[offset])
            } else {
                f64::from(record[offset] as i8)
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original block settings observations and script"]
    fn matches_native_default_block_settings() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BLOCK_SETTINGS")).unwrap();
        assert_eq!(&bytes[..8], b"MTBLKSET");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 52);
        let mut definitions = Definitions {
            rows: std::array::from_fn(|i| {
                bytes[16 + i * 0x58..16 + (i + 1) * 0x58]
                    .try_into()
                    .unwrap()
            }),
        };
        let after_definitions = 16 + 52 * 0x58;
        let before_categories = after_definitions + 52 * 0x58;
        let mut categories = std::array::from_fn(|i| {
            bytes[before_categories + i * 0x17c..before_categories + (i + 1) * 0x17c]
                .try_into()
                .unwrap()
        });
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let defaults = crate::fixtures::path("DEFAULT_SETTINGS");
        let script = std::fs::read(defaults.parent().unwrap().join("blocksettings.lua")).unwrap();
        let mut runtime = Runtime::new().unwrap();
        let bindings = Bindings::register(&mut runtime, 100).unwrap();
        Bindings::register_block_constants(&mut runtime, &image).unwrap();
        let mut cash = 0.;
        runtime
            .execute("original blocksettings", &script, &mut |id, args| {
                bindings.handle(
                    id,
                    args,
                    &mut State {
                        num_cars: &mut 0,
                        categories: &mut categories,
                        player_money: &mut cash,
                        definitions: &mut definitions,
                    },
                )
            })
            .unwrap();
        for (i, row) in definitions.rows.iter().enumerate() {
            assert_eq!(
                row.as_slice(),
                &bytes[after_definitions + i * 0x58..after_definitions + (i + 1) * 0x58],
                "descriptor {i}"
            );
        }
        let after_categories = before_categories + 52 * 0x17c;
        for (i, row) in categories.iter().enumerate() {
            assert_eq!(
                row.as_slice(),
                &bytes[after_categories + i * 0x17c..after_categories + (i + 1) * 0x17c],
                "category {i}"
            );
        }
        assert_eq!(after_categories + 52 * 0x17c, bytes.len());
    }

    #[test]
    #[ignore = "requires supplied SCENARIO3 scripts and v1.4 executable"]
    fn original_starting_out_hub_runs_nested_map_and_preferences_in_order() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let ai = crate::fixtures::path("AI_SCRIPT");
        let root = ai.parent().unwrap().parent().unwrap();
        let hub = std::fs::read(root.join("SCENARIO3/hub.lua")).unwrap();
        let map = std::fs::read(root.join("maps/apartmentrim.lua")).unwrap();
        let prefs = std::fs::read(root.join("SCENARIO3/initialpref.lua")).unwrap();
        let expected_map =
            crate::scene::parse_static_map(std::str::from_utf8(&map).unwrap()).unwrap();
        for difficulty in 0..3 {
            let mut runtime = Runtime::new().unwrap();
            let bindings = Bindings::register(&mut runtime, 100).unwrap();
            Bindings::register_block_constants(&mut runtime, &image).unwrap();
            for (name, value) in [
                ("DIFFICULTY_EASY", 0.),
                ("DIFFICULTY_MEDIUM", 1.),
                ("DIFFICULTY_HARD", 2.),
                ("OBJECTIVE_SALES", 6.),
            ] {
                runtime.set_number(name, value).unwrap();
            }
            for (name, id) in [
                ("GetDifficulty", 1),
                ("AddBuilding", 2),
                ("AddBusiness", 3),
                ("SetObjective", 4),
                ("RegisterPrompt", 5),
            ] {
                runtime.register(name, id).unwrap();
            }
            crate::population_script::register(&mut runtime, 6).unwrap();
            runtime.register_script_loader("C_RunScript").unwrap();
            runtime
                .script_buffer("scripts/maps/apartmentrim.lua", &map)
                .unwrap();
            runtime
                .script_buffer("scripts/scenario3/initialpref.lua", &prefs)
                .unwrap();
            runtime
                .execute(
                    "Game bindings",
                    b"Game={GetDifficulty=GetDifficulty,SetObjective=SetObjective}",
                    &mut |_, _| Err("unexpected binding callback".into()),
                )
                .unwrap();
            let mut categories = [[0xa5; 0x17c]; 52];
            let mut cash = crate::player_manager::Manager::initial_cash(&image).unwrap();
            let expected_cash = [15000., 12000., 10000.][difficulty];
            let mut definitions = Definitions::from_v14(&image).unwrap();
            let mut state = State {
                num_cars: &mut 0,
                categories: &mut categories,
                player_money: &mut cash,
                definitions: &mut definitions,
            };
            let mut commands = vec![];
            let mut building = 0;
            let mut handler = |id, args: &[Value]| {
                if bindings.contains(id) {
                    return bindings.handle(id, args, &mut state);
                }
                if id == 1 {
                    return Ok(Some(difficulty as f64));
                }
                commands.push((id, args.to_vec()));
                match id {
                    2 => {
                        let index = building;
                        building += 1;
                        Ok(Some(index as f64))
                    }
                    6 => {
                        assert_eq!(*state.player_money, expected_cash);
                        assert!(state.categories.iter().all(|r| r[0x74] == 0xa5));
                        assert_eq!(
                            crate::population_script::Request::from_arguments(args)?,
                            crate::population_script::Request {
                                count: [423, 340, 280][difficulty],
                                identity: [-1; 3]
                            }
                        );
                        Ok(None)
                    }
                    3..=5 => Ok(None),
                    _ => Err("unrecovered level callback".into()),
                }
            };
            runtime
                .execute("scenario3/hub.lua", &hub, &mut handler)
                .unwrap();
            runtime
                .call("EVENTINITIALISE_Level", &[0.], &mut handler)
                .unwrap();
            runtime.call("EVENT_Level", &[0.], &mut handler).unwrap();
            assert_eq!(building, 9);
            // The callbacks are observational seams here, not constructed city
            // or population objects. Assert the complete supplied map arguments.
            for (i, spec) in expected_map.iter().enumerate() {
                let mut args = vec![Value::Bytes(spec.block.as_bytes().to_vec())];
                args.extend(spec.lot.map(|v| Value::Number(f64::from(v))));
                args.push(Value::Number(f64::from(spec.height)));
                args.push(Value::Bytes(b"RESIDENTIAL".to_vec()));
                args.push(Value::Number(f64::from(spec.variation)));
                args.push(Value::Bytes(
                    [b"low".as_slice(), b"medium", b"high"][spec.quality as usize].to_vec(),
                ));
                assert_eq!(commands[i * 2], (2, args));
                assert_eq!(
                    commands[i * 2 + 1],
                    (
                        3,
                        vec![
                            Value::Number(i as f64),
                            Value::Bytes(b"apartments".to_vec())
                        ]
                    )
                );
            }
            assert_eq!(commands[18].0, 6);
            assert_eq!(
                commands[19],
                (
                    4,
                    vec![Value::Number(6.), Value::Number(100.), Value::Number(0.)]
                )
            );
            assert_eq!(commands.len(), if difficulty == 0 { 29 } else { 20 });
            assert_eq!(cash, expected_cash);
            assert!(categories.iter().all(|r| r[0x74] == 0 && r[5] == 0xa5));
            assert_eq!(categories.iter().filter(|r| r[0x78] == 2).count(), 25);
            assert!(categories.iter().all(|r| [0, 2].contains(&r[0x78])));
            // The misspelling is the actual registered identifier.
            assert_eq!(runtime.number("PENSYLVANIA_RAILROAD").unwrap(), Some(43.));
        }
    }
    #[test]
    fn property_writes_preserve_raw_neighbors_and_native_read_types() {
        let mut runtime = Runtime::new().unwrap();
        let bindings = Bindings::register(&mut runtime, 20).unwrap();
        let mut categories = [[0xa5; 0x17c]; 52];
        let mut cash = 5000.;
        let mut definitions = Definitions {
            rows: [[0xa5; 0x58]; 52],
        };
        let mut state = State {
            num_cars: &mut 0,
            categories: &mut categories,
            player_money: &mut cash,
            definitions: &mut definitions,
        };
        runtime.execute("properties",b"Block[0].buildingrights=258.9; Block[0].owner=-1.9; Block[0].ownable=false; InitialValue.playermoney='12000.5'; rights=Block[0].buildingrights; owner=Block[0].owner; cash=InitialValue.playermoney; if Block[0].ownable then numeric_zero_is_truthy=1 end; if false then false_is_truthy=1 end", &mut |id,args|bindings.handle(id,args,&mut state)).unwrap();
        let mut expected = [[0xa5; 0x17c]; 52];
        expected[0][0x78] = 2;
        expected[0][5] = 255;
        expected[0][0x74] = 0;
        assert_eq!(categories, expected);
        assert_eq!(cash, 12000.5);
        assert_eq!(runtime.number("rights").unwrap(), Some(2.));
        assert_eq!(runtime.number("owner").unwrap(), Some(-1.));
        assert_eq!(runtime.number("cash").unwrap(), Some(12000.5));
        assert_eq!(runtime.number("numeric_zero_is_truthy").unwrap(), Some(1.));
        assert_eq!(runtime.number("false_is_truthy").unwrap(), None);
    }
}
