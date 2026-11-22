//! Original4dce90 People.Create callback feeding the recovered resident constructor.
use crate::lua4::{Runtime, Value};

/// Original4dcf90 FILDs the signed pool count7a18ec and pushes a Lua number.
pub fn number_in_city(active_count: u32) -> f64 {
    f64::from(active_count as i32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    /// Original loop skips only zero; negative signed inputs wrap to this count.
    pub count: u32,
    pub identity: [i32; 3],
}

impl Request {
    /// The registered bridge has already applied Lua tonumber to argument one.
    /// Unlike the count, optional identity fields require actual numeric types.
    pub fn from_arguments(arguments: &[Value]) -> Result<Self, String> {
        let Some(Value::Number(count)) = arguments.first() else {
            return Err("People.Create requires its numeric-first Lua bridge".into());
        };
        Ok(Self {
            count: crate::economy::ftol_low_word(*count) as u32,
            identity: std::array::from_fn(|i| match arguments.get(i + 1) {
                Some(Value::Number(value)) => crate::economy::ftol_low_word(*value),
                _ => -1,
            }),
        })
    }

    ///4dcf17..4dcf31: fixed-count calls, independent of allocation success.
    /// Host errors may stop execution; no silent count or pool-capacity clamp.
    pub fn dispatch<E>(self, mut create: impl FnMut([i32; 3]) -> Result<(), E>) -> Result<(), E> {
        for _ in 0..self.count {
            create(self.identity)?;
        }
        Ok(())
    }
}

/// Install the original function shape without replacing other People entries.
/// The owner routes `callback_id` to Request::from_arguments and Population::create.
pub fn register(runtime: &mut Runtime, callback_id: u32) -> Result<(), String> {
    runtime.execute(
        "People table",
        b"if not People then People={} end",
        &mut |_, _| Err("unexpected binding callback".into()),
    )?;
    runtime.register_number_first_method("People", "Create", callback_id)
}

pub struct Population<
    'a,
    C: crate::resident::PlacementCategories + ?Sized = [crate::housing::Category; 52],
> {
    pub residents: &'a mut crate::economy::ResidentAccounts,
    pub inputs: &'a crate::economy::ResidentCreationInputs<'a>,
    pub state: &'a mut crate::economy::ResidentCreationState,
    pub categories: &'a mut C,
    pub auxiliary_template: [u8; 0x11c],
    /// Live801dec gate for optional471c30 after model binding.
    pub creation_events_enabled: bool,
}

impl<C: crate::resident::PlacementCategories + ?Sized> Population<'_, C> {
    /// Each -1 identity component is resolved inside each constructor, consuming
    /// that resident's RNG draws. The optional event runs after model binding.
    pub fn create(
        &mut self,
        request: Request,
        on_created: &mut impl FnMut(
            usize,
            &mut Self,
        ) -> Result<(), crate::economy::ResidentCreationError>,
    ) -> Result<(), crate::economy::ResidentCreationError> {
        request.dispatch(|identity| {
            if let Some(index) = self.residents.create_resident(
                identity,
                self.auxiliary_template,
                self.inputs,
                self.state,
                self.categories,
            )? && self.creation_events_enabled
            {
                on_created(index, self)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original campaign-language and population query fixture"]
    fn original_campaign_event_bindings() {
        let fixture =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CAMPAIGN_EVENTS").unwrap()).unwrap();
        let executable = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&executable).unwrap();
        let mut vm = Runtime::new().unwrap();
        crate::level_catalog::script::register_language(&mut vm, &image).unwrap();
        let word = |at| u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
        assert_eq!(&fixture[..8], b"MTCPEV02");
        assert_eq!(word(8), 0x37f);
        let mut at = 24;
        for _ in 0..word(12) {
            let n = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&fixture[at..at + n]).unwrap();
            at += n;
            let value = f32::from_bits(word(at));
            at += 4;
            assert_eq!(vm.number(name).unwrap(), Some(f64::from(value)), "{name}");
        }
        register(&mut vm, 1).unwrap();
        vm.register_number_method("People", "NumberInCity", 2, 0)
            .unwrap();
        for _ in 0..word(16) {
            let count = word(at);
            let expected = f64::from_le_bytes(fixture[at + 4..at + 12].try_into().unwrap());
            at += 12;
            vm.execute(
                "population query",
                b"population=People.NumberInCity()",
                &mut |id, _| {
                    assert_eq!(id, 2);
                    Ok(Some(number_in_city(count)))
                },
            )
            .unwrap();
            assert_eq!(vm.number("population").unwrap(), Some(expected));
        }
        for _ in 0..word(20) {
            let mut state =
                crate::scenario_objective::State::from_bytes(&fixture[at..at + 60]).unwrap();
            state.reset();
            assert_eq!(state.to_bytes().as_slice(), &fixture[at + 60..at + 120]);
            at += 120;
        }
        assert_eq!(at, fixture.len());
        assert_eq!(crate::scenario_objective::State::default().flag, 1);
    }
    #[test]
    #[ignore = "requires original configuration, scripts and people models"]
    fn starting_out_preferences_construct_real_population_without_a_save() {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let text = |name| String::from_utf8(read(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let config = crate::resident::Configuration::from_sources(
            &text("OPENMT2001_DEFAULT_SETTINGS"),
            &text("OPENMT2001_PARAMETERS"),
            &text("OPENMT2001_TIMETABLE"),
            &image,
        )
        .unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = crate::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let models = crate::people::PeopleModels::load(&image, &directory, &archive).unwrap();
        let handles = std::array::from_fn(|i| [(i * 2 + 1) as u32, (i * 2 + 2) as u32]);
        let bindings = models.resident_bindings(&handles).unwrap();
        let geometry = crate::board::Board::from_v14(&image)
            .unwrap()
            .category_geometry();
        let default_path =
            std::path::PathBuf::from(std::env::var("OPENMT2001_DEFAULT_SETTINGS").unwrap());
        let prefs = std::fs::read(
            default_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("SCENARIO3/initialpref.lua"),
        )
        .unwrap();
        for difficulty in 0..3 {
            let expected = [423, 340, 280][difficulty];
            let mut runtime = Runtime::new().unwrap();
            let properties =
                crate::initial_conditions::Bindings::register(&mut runtime, 100).unwrap();
            crate::initial_conditions::Bindings::register_block_constants(&mut runtime, &image)
                .unwrap();
            let commodity_bindings =
                crate::commodity_script::Bindings::register(&mut runtime, &image, 300).unwrap();
            let mut commodity_state =
                crate::commodity_script::State::from_v14(&image, [0; 5]).unwrap();
            runtime
                .execute(
                    "commoditysettings.lua",
                    &read("OPENMT2001_COMMODITY_SETTINGS"),
                    &mut |id, args| commodity_bindings.handle(id, args, &mut commodity_state),
                )
                .unwrap();
            let commodities = commodity_state.resident_inputs();
            // Explicit hour and RNG inputs: this is not a recovered fresh-game
            // seed policy. No saved resident/configuration/model bytes are used.
            let personal = config.inputs(&commodities, commodity_state.memberships, 9);
            let inputs = crate::economy::ResidentCreationInputs {
                personal: &personal,
                preference_groups: &config.preference_groups,
                models: &bindings,
                countdown_period: 2,
            };
            let mut residents = crate::economy::ResidentAccounts::new_pool();
            residents.reset_account_pool().unwrap();
            let mut creation = crate::economy::ResidentCreationState {
                auxiliary: crate::resident::AuxiliaryPool::new(2000),
                initialization_rng: crate::resident::InitializationRng {
                    constructor: 1,
                    movement: 2,
                    objective: 3,
                },
                identity_rng: 4,
                preference_rng: 5,
            };
            let mut categories = std::array::from_fn(|i| crate::housing::Category {
                buildings: vec![],
                value: 0.,
                center: Some(geometry.centers[i]),
                resident_counter: 0,
            });
            let mut raw_categories = [[0; 0x17c]; 52];
            let mut cash = 5000.;
            let mut definitions = crate::initial_conditions::Definitions::from_v14(&image).unwrap();
            for (name, value) in [
                ("DIFFICULTY_EASY", 0.),
                ("DIFFICULTY_MEDIUM", 1.),
                ("DIFFICULTY_HARD", 2.),
            ] {
                runtime.set_number(name, value).unwrap();
            }
            register(&mut runtime, 6).unwrap();
            runtime.register("GetDifficulty", 7).unwrap();
            runtime
                .execute(
                    "Game binding",
                    b"Game={GetDifficulty=GetDifficulty}",
                    &mut |_, _| Err("unexpected binding callback".into()),
                )
                .unwrap();
            {
                let mut state = crate::initial_conditions::State {
                    num_cars: &mut 0,
                    categories: &mut raw_categories,
                    player_money: &mut cash,
                    definitions: &mut definitions,
                };
                let mut population = Population {
                    residents: &mut residents,
                    inputs: &inputs,
                    state: &mut creation,
                    categories: &mut categories,
                    auxiliary_template: [0xa5; 0x11c],
                    creation_events_enabled: false,
                };
                runtime
                    .execute("scenario3/initialpref.lua", &prefs, &mut |id, args| {
                        if properties.contains(id) {
                            return properties.handle(id, args, &mut state);
                        }
                        match id {
                            7 => Ok(Some(difficulty as f64)),
                            6 => {
                                population
                                    .create(Request::from_arguments(args)?, &mut |_, _| {
                                        panic!("creation event disabled")
                                    })
                                    .map_err(|e| format!("resident creation: {e:?}"))?;
                                Ok(None)
                            }
                            _ => Err("unknown initialization callback".into()),
                        }
                    })
                    .unwrap();
            }
            assert_eq!(residents.active_count, expected as u32);
            assert_eq!(
                residents.nodes.iter().filter(|n| n.is_active()).count(),
                expected
            );
            assert_eq!(
                residents.accounts.iter().filter(|n| n.is_active()).count(),
                expected
            );
            assert_eq!(creation.auxiliary.used(), expected);
            assert_eq!(
                categories.iter().map(|c| c.resident_counter).sum::<u32>(),
                expected as u32
            );
            for (i, node) in residents.nodes[..expected].iter().enumerate() {
                assert_eq!(node.pool_index(), i as u16);
                assert_eq!(node.account_index(), i as i16);
                assert_eq!(node.auxiliary_reference(), i as u32 + 1);
                assert_eq!(node.housing_building(), -1);
                //45c750 assigns tourists a station category without assigning
                //a housing building; other ages retain the -1 category.
                if node.age() == 4 {
                    assert!([4, 17, 43, 44].contains(&node.housing_category()));
                } else {
                    assert_eq!(node.housing_category(), -1);
                }
                assert!(node.age() < 5 && node.class() < 3 && node.preference_index() >= 0);
                assert!(residents.accounts[i].balance().is_finite());
            }
            assert_eq!(cash, [15000., 12000., 10000.][difficulty]);
            assert_eq!(raw_categories.iter().filter(|r| r[0x78] == 2).count(), 25);
        }
    }

    #[test]
    #[ignore = "requires original People.Create callback observations"]
    fn matches_native_callback_calls_through_lua() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PEOPLE_CREATE_CALLBACK").unwrap())
                .unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTPCALL1");
        assert_eq!(word(8), 1);
        let mut at = 16;
        let mut runtime = Runtime::new().unwrap();
        register(&mut runtime, 7).unwrap();
        for _ in 0..word(12) {
            let name_len = word(at) as usize;
            let count = word(at + 4) as usize;
            at += 8;
            let name = std::str::from_utf8(&bytes[at..at + name_len]).unwrap();
            at += name_len;
            let mut args = vec![];
            for _ in 0..4 {
                let value = f64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
                let kind = word(at + 8);
                at += 12;
                args.push(match kind {
                    1 => "nil".into(),
                    2 => format!("{value}"),
                    3 => format!("'{value}'"),
                    4 => "{}".into(),
                    _ => panic!("unknown fixture type"),
                });
            }
            assert_eq!(word(at), 1);
            assert_eq!(word(at + 4), 0x100f0000);
            assert_eq!(word(at + 8), 0);
            at += 12;
            let mut expected = vec![];
            for _ in 0..count {
                expected.push([word(at) as i32, word(at + 4) as i32, word(at + 8) as i32]);
                at += 12;
            }
            let mut calls = vec![];
            runtime
                .execute(
                    name,
                    format!("People.Create({})", args.join(",")).as_bytes(),
                    &mut |_, args| {
                        Request::from_arguments(args)?.dispatch::<String>(|identity| {
                            calls.push(identity);
                            Ok(())
                        })?;
                        Ok(None)
                    },
                )
                .unwrap();
            assert_eq!(calls, expected, "{name}");
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    fn lua_count_coercion_optional_types_and_interruptible_wrapping_count() {
        let mut runtime = Runtime::new().unwrap();
        register(&mut runtime, 7).unwrap();
        let mut requests = vec![];
        runtime.execute("population arguments", b"People.Create('3.9',2.9,'1',nil); People.Create(); People.Create('bad'); People.Create(-1,0,1,2)", &mut |id,args| {
            assert_eq!(id,7);
            requests.push(Request::from_arguments(args)?);
            Ok(None)
        }).unwrap();
        assert_eq!(
            requests[0],
            Request {
                count: 3,
                identity: [2, -1, -1]
            }
        );
        assert_eq!(requests[1].count, 0);
        assert_eq!(requests[2].count, 0);
        assert_eq!(
            requests[3],
            Request {
                count: u32::MAX,
                identity: [0, 1, 2]
            }
        );
        let mut calls = vec![];
        requests[0]
            .dispatch::<()>(|identity| {
                calls.push(identity);
                Ok(())
            })
            .unwrap();
        assert_eq!(calls, vec![[2, -1, -1]; 3]);
        let mut visits = 0;
        assert_eq!(
            requests[3].dispatch(|_| {
                visits += 1;
                Err("host stop")
            }),
            Err("host stop")
        );
        assert_eq!(visits, 1);
    }
}
