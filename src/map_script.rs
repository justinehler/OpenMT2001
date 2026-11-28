//! Original AddBuilding/AddBusiness callbacks over caller-owned live pools.
//! Model/notification/accounting dependencies are explicit, never skipped.
use crate::{
    building_pool, business_binding, business_pool,
    lua4::{Runtime, Value},
    pe::PeImage,
};

fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}
fn name(value: Option<&Value>) -> Result<Vec<u8>, String> {
    let Some(Value::Bytes(bytes)) = value else {
        return Err("native map name requires a Lua string-convertible value".into());
    };
    // Native lookups are C strings, and supplied identifiers are ASCII.
    Ok(bytes
        .iter()
        .copied()
        .take_while(|&b| b != 0)
        .map(|b| b.to_ascii_uppercase())
        .collect())
}
fn number(args: &[Value], at: usize) -> Result<i32, String> {
    match args.get(at) {
        Some(Value::Number(n)) => Ok(crate::economy::ftol_low_word(*n)),
        _ => Err("map callback requires Lua-coerced numeric arguments".into()),
    }
}
fn lookup(
    image: &PeImage<'_>,
    name: &[u8],
    base: u32,
    stride: u32,
    count: u32,
    length: usize,
) -> Result<Option<u32>, String> {
    let mut found = None;
    for i in 0..count {
        let row = image.at(base + i * stride, length)?;
        let end = row
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated original map name")?;
        if row[..end].eq_ignore_ascii_case(name) {
            found = Some(i);
        }
    }
    Ok(found)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Building {
    pub category: usize,
    pub rectangle: [i32; 4],
    pub height: i32,
    pub volume: i32,
    pub kind: u32,
    pub variation: u32,
    pub quality: i8,
}
impl Building {
    pub fn from_arguments(args: &[Value], image: &PeImage<'_>) -> Result<Option<Self>, String> {
        let Some(category) = lookup(image, &name(args.first())?, 0x5385a5, 0x58, 52, 64)? else {
            return Ok(None);
        };
        let [x, y, width, depth, height] = [
            number(args, 1)?,
            number(args, 2)?,
            number(args, 3)?,
            number(args, 4)?,
            number(args, 5)?,
        ];
        let Some(kind) = lookup(image, &name(args.get(6))?, 0x5365d0, 0x2e, 3, 32)? else {
            return Ok(None);
        };
        let variation = number(args, 7)? as u32;
        let Some(quality) = lookup(image, &name(args.get(8))?, 0x53a6b8, 0x54, 3, 32)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            category: category as usize,
            rectangle: [
                x,
                y,
                x.wrapping_add(width).wrapping_sub(1),
                y.wrapping_add(depth).wrapping_sub(1),
            ],
            height,
            volume: height.wrapping_mul(width).wrapping_mul(depth),
            kind,
            variation,
            quality: quality as i8,
        }))
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct Business {
    pub building: i32,
    pub kind: u32,
}
impl Business {
    pub fn from_arguments(
        args: &[Value],
        catalog: &business_pool::BusinessCatalog,
    ) -> Result<Option<Self>, String> {
        let building = number(args, 0)?;
        if building == -1 {
            return Ok(None);
        }
        let name = name(args.get(1))?;
        let mut kind = None;
        for row in catalog.rows() {
            let end = row[0x10..]
                .iter()
                .position(|&b| b == 0)
                .ok_or("unterminated business name")?;
            if row[0x10..0x10 + end].eq_ignore_ascii_case(&name) {
                kind = Some(word(row, 0));
            }
        }
        Ok(kind
            .filter(|&kind| kind != u32::MAX)
            .map(|kind| Self { building, kind }))
    }
}

pub struct Bindings {
    building: u32,
    business: u32,
}
impl Bindings {
    pub fn register(runtime: &mut Runtime, building: u32, business: u32) -> Result<Self, String> {
        if building == business {
            return Err("map callback IDs overlap".into());
        }
        runtime.register_coerced("AddBuilding", building, "snnnnnsns")?;
        runtime.register_coerced("AddBusiness", business, "ns")?;
        Ok(Self { building, business })
    }
    pub fn contains(&self, id: u32) -> bool {
        id == self.building || id == self.business
    }
    pub fn handle(
        &self,
        id: u32,
        args: &[Value],
        state: &mut State<'_, '_>,
        geometry: &mut impl building_pool::GeometryBinding,
        hooks: &mut impl Hooks,
    ) -> Result<Option<f64>, String> {
        if id == self.building {
            match Building::from_arguments(args, state.image)? {
                Some(request) => state
                    .add_building(&request, geometry, hooks)
                    .map(|slot| Some(slot.map_or(-1., |n| n as f64))),
                None => Ok(None),
            }
        } else if id == self.business {
            if let Some(request) = Business::from_arguments(args, state.world.catalog)? {
                state.add_business(&request, hooks)?;
            }
            Ok(None)
        } else {
            Err("unknown map callback".into())
        }
    }
}

/// Remaining original external calls; presentation must be explicitly handled
/// even when the caller chooses a headless world.
pub trait Hooks: business_pool::BusinessPoolHooks {
    fn building_appearance(&mut self, slot: usize, record: &mut [u8; 0x394]) -> Result<(), String>;
    fn building_created(&mut self, slot: usize) -> Result<(), String>;
    fn binding_call(
        &mut self,
        call: business_binding::Call,
        world: &mut business_binding::World<'_>,
    ) -> Result<(), String>;
    fn stock_charge(&mut self, business: &mut [u8; 0x220], amount: f32) -> Result<(), String>;
}
pub struct State<'a, 'image> {
    pub building_parameters: crate::building_parameters::BuildingKinds,
    pub world: business_binding::World<'a>,
    pub runtime: business_binding::Runtime<'a>,
    pub image: &'a PeImage<'image>,
    pub hour: i32,
    pub discount: f32,
    pub date: [u8; 2],
    /// Original822d44, distinct from the initializer's822d40 timestamp.
    pub ownership_timestamp: u32,
    pub utility_owners: [u32; 4],
    pub notifications: bool,
    /// Original6175fc/617600/617604/617608, retained for live cost consumers.
    pub construction_components: [f32; 4],
}
impl State<'_, '_> {
    pub fn add_building(
        &mut self,
        request: &Building,
        geometry: &mut impl building_pool::GeometryBinding,
        hooks: &mut impl Hooks,
    ) -> Result<Option<i32>, String> {
        let category = self
            .world
            .categories
            .get(request.category)
            .ok_or("map category leaves table")?;
        if !building_pool::can_place(category[0x9c..0xcd].try_into().unwrap(), request.rectangle) {
            return Ok(None);
        }
        let Some(slot) = building_pool::first_free(self.world.buildings)? else {
            return Ok(None);
        };
        // AddBuilding passes BusinessInfo1 and owner0 to445720, regardless of
        // the business subsequently attached to this building.
        let info = self
            .world
            .catalog
            .row(1)
            .ok_or("missing construction business row")?;
        let mut products = Vec::new();
        if self.hour >= 6 && request.kind == 0 {
            for entry in self.runtime.products[1]
                .get(..usize::from(info[0x4c]))
                .ok_or("construction catalog exceeds six")?
            {
                let row = self
                    .runtime
                    .commodities
                    .get(entry[0] as usize)
                    .ok_or("construction commodity leaves table")?;
                products.push(crate::building_binding::StockProduct {
                    value: f32::from_bits(word(row, 0x14)),
                    fraction: f32::from_bits(entry[1]),
                });
            }
        }
        let cost = crate::building_binding::construction_cost_with_parameters(
            &crate::building_binding::ConstructionInputs {
                volume: request.volume,
                kind: request.kind,
                quality: request.quality,
                category_value: f32::from_bits(word(category, 0x84)),
                hour: self.hour,
                discount: self.discount,
                owner: 0,
                utility_owners: self.utility_owners,
                business: info,
                products: &products,
            },
            &self.building_parameters,
            self.image,
        )?;
        self.construction_components = cost.components;
        let center = building_pool::center(category, request.rectangle, self.image)?;
        building_pool::initialize_with_parameters(
            &mut self.world.buildings[slot],
            &building_pool::Initialization {
                rectangle: request.rectangle,
                height: request.height as u32,
                center,
                variation: request.variation,
                kind: request.kind,
                quality: request.quality,
                total_cost: cost.building_argument_bits(),
                date: self.date,
            },
            &self.building_parameters,
        )?;
        let reference = self
            .world
            .category_base
            .checked_add((request.category * 0x17c) as u32)
            .ok_or("category reference overflow")?;
        building_pool::bind_category(
            &mut self.world.categories[request.category],
            self.world.buildings,
            slot,
            self.runtime.building_base,
            reference,
            geometry,
        )?;
        let record = &mut self.world.buildings[slot];
        record[0x2d] = self.world.categories[request.category][5];
        record[0x30..0x34].copy_from_slice(&self.ownership_timestamp.to_le_bytes());
        if word(record, 0x4c) == 1 {
            hooks.building_appearance(slot, record)?;
        }
        let identity = word(record, 0) as i32;
        if identity > -1 {
            hooks.building_created(identity as usize)?;
        }
        Ok(Some(identity))
    }
    pub fn add_business(
        &mut self,
        request: &Business,
        hooks: &mut impl Hooks,
    ) -> Result<(), String> {
        let index = usize::try_from(request.building).map_err(|_| "negative map building index")?;
        if index >= self.world.buildings.len() {
            return Err("map building index leaves pool".into());
        }
        let records = (&mut *self.world.businesses)
            .try_into()
            .map_err(|_| "map requires600 business slots")?;
        let Some(slot) = business_pool::BusinessPool::new(records).allocate(
            request.kind,
            0,
            request.building as u16,
            self.world.catalog,
            hooks,
        )?
        else {
            return Ok(());
        };
        let hooks = std::cell::RefCell::new(hooks);
        self.runtime.bind(
            &mut self.world,
            &business_binding::Inputs {
                business: slot,
                building: index,
                stock_argument: 1,
                hour: self.hour,
                discount: self.discount,
                notifications: self.notifications,
            },
            self.image,
            &mut |call, world| hooks.borrow_mut().binding_call(call, world),
            &mut |record, amount| hooks.borrow_mut().stock_charge(record, amount),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Headless {
        calls: Vec<(u32, usize)>,
        utilities: crate::economy::Utilities,
        objective: Option<Vec<Value>>,
        prompts: Vec<Vec<Value>>,
        paused: bool,
        frequency: crate::timers::CalendarFrequency,
        timers: Vec<crate::timers::Task>,
    }
    impl crate::level_script::Hooks for Headless {
        fn calendar_timers(
            &mut self,
        ) -> (
            &mut crate::timers::CalendarFrequency,
            &mut [crate::timers::Task],
        ) {
            (&mut self.frequency, &mut self.timers)
        }
        fn pause_timers(&mut self, event: crate::pause::Timers) -> Result<(), String> {
            self.paused = event == crate::pause::Timers::Suspend;
            event.apply(&mut self.timers);
            Ok(())
        }
        fn command(
            &mut self,
            command: crate::level_script::Command,
            arguments: &[Value],
        ) -> Result<(), String> {
            match command {
                crate::level_script::Command::Objective => {
                    self.objective = Some(arguments.to_vec())
                }
                crate::level_script::Command::Prompt => self.prompts.push(arguments.to_vec()),
                crate::level_script::Command::PlaySound => {}
                crate::level_script::Command::ResultFlyby
                | crate::level_script::Command::TutorialDisableAllControls
                | crate::level_script::Command::TutorialDisableAllPicking
                | crate::level_script::Command::TutorialEnableAllPicking
                | crate::level_script::Command::TutorialPositionPrompt
                | crate::level_script::Command::TutorialEnableBlockPicking
                | crate::level_script::Command::TutorialEnableBuildingPicking
                | crate::level_script::Command::TutorialDisableCameraControl
                | crate::level_script::Command::TutorialArrow
                | crate::level_script::Command::TutorialMessage
                | crate::level_script::Command::TutorialDismissPrompt
                | crate::level_script::Command::TutorialEnableControl
                | crate::level_script::Command::TutorialEnableControlAndChildren
                | crate::level_script::Command::TutorialDisableControl
                | crate::level_script::Command::TutorialCameraMode
                | crate::level_script::Command::TutorialCityView
                | crate::level_script::Command::TutorialPlanView
                | crate::level_script::Command::TutorialBuildType
                | crate::level_script::Command::TutorialBuildUse
                | crate::level_script::Command::CreateEventList
                | crate::level_script::Command::ForcedAuction { .. }
                | crate::level_script::Command::ResultCameraMode
                | crate::level_script::Command::ResultFireworks
                | crate::level_script::Command::ResultCutSceneIdentifier
                | crate::level_script::Command::ResultEnableBypass
                | crate::level_script::Command::ResultDisableBypass
                | crate::level_script::Command::ResultCredits
                | crate::level_script::Command::ResultEnd => {
                    return Err("level presentation called during map-script fixture".into());
                }
            }
            Ok(())
        }
    }
    impl business_pool::BusinessPoolHooks for Headless {
        fn owner_transfer(
            &mut self,
            slot: usize,
            record: &mut [u8; 0x220],
            owner: u32,
        ) -> Result<(), String> {
            // Explicit fixture gate8b22dc=0:462b00 only stores owner byte.
            assert_eq!(owner, 0);
            record[0xa] = owner as u8;
            self.calls.push((0x462b00, slot));
            Ok(())
        }
        fn notify(&mut self) -> Result<(), String> {
            self.calls.push((0x41f410, 0));
            Ok(())
        }
    }
    impl Hooks for Headless {
        fn building_appearance(&mut self, slot: usize, _: &mut [u8; 0x394]) -> Result<(), String> {
            self.calls.push((0x485520, slot));
            Ok(())
        }
        fn building_created(&mut self, slot: usize) -> Result<(), String> {
            self.calls.push((0x471d50, slot));
            Ok(())
        }
        fn binding_call(
            &mut self,
            call: business_binding::Call,
            world: &mut business_binding::World<'_>,
        ) -> Result<(), String> {
            match call {
                business_binding::Call::ChargeUtilities { business } => {
                    world.assess_binding_utilities(business, &mut self.utilities)?;
                    self.calls.push((0x462070, business));
                }
                business_binding::Call::PayUtilityOwners => {
                    // The startup map has no utility owners. Execute the recovered
                    // payout/reset body, rather than omit the call at hour9.
                    self.utilities.pay_owners(&mut [], 1.)?;
                    self.calls.push((0x48dfc0, 0));
                }
                business_binding::Call::RefreshBuilding { building } => {
                    self.calls.push((0x4450a0, building))
                }
                _ => return Err(format!("unexpected map startup call {call:?}")),
            };
            Ok(())
        }
        fn stock_charge(&mut self, _: &mut [u8; 0x220], _: f32) -> Result<(), String> {
            Err("unexpected map initializer cash charge".into())
        }
    }

    #[test]
    #[ignore = "requires original native map callbacks and executable"]
    fn matches_native_map_callback_contracts() {
        struct Reader {
            data: Vec<u8>,
            at: usize,
        }
        impl Reader {
            fn bytes<const N: usize>(&mut self) -> [u8; N] {
                let b = self.data[self.at..self.at + N].try_into().unwrap();
                self.at += N;
                b
            }
            fn word(&mut self) -> u32 {
                u32::from_le_bytes(self.bytes())
            }
        }
        struct Geometry;
        impl building_pool::GeometryBinding for Geometry {
            fn prepare(&mut self, _: usize, r: &mut [u8; 0x394]) -> Result<(), String> {
                r[0x170..0x174].copy_from_slice(&1_u32.to_le_bytes());
                Ok(())
            }
            fn model_height(&mut self, _: u32) -> Result<f32, String> {
                Ok(10.)
            }
        }
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut reader = Reader {
            data: std::fs::read(crate::fixtures::path("NATIVE_MAP_CALLBACKS")).unwrap(),
            at: 0,
        };
        assert_eq!(reader.bytes::<8>(), *b"MTMAPC01");
        for case in 0..reader.word() {
            let id = reader.word();
            let len = reader.word() as usize;
            let source = reader.data[reader.at..reader.at + len].to_vec();
            reader.at += len;
            let returns = reader.word();
            let returned = f64::from_le_bytes(reader.bytes());
            let constructor = if reader.word() != 0 {
                Some(std::array::from_fn::<_, 11, _>(|_| reader.word()))
            } else {
                None
            };
            let native_calls: Vec<[u32; 4]> = (0..reader.word())
                .map(|_| std::array::from_fn(|_| reader.word()))
                .collect();
            let catalog = business_pool::BusinessCatalog::from_pe(&image).unwrap();
            let mut buildings = vec![[0; 0x394]; 600];
            let mut businesses: Box<[[u8; 0x220]; 600]> =
                vec![[0; 0x220]; 600].into_boxed_slice().try_into().unwrap();
            business_pool::BusinessPool::new(&mut businesses).reset_records();
            let mut categories = [[0; 0x17c]; 52];
            let mut available = vec![];
            let mut cursor = None;
            // Downstream AddBusiness calls are native oracle seams. Supply a
            // valid unowned target to run their Rust implementation as well.
            if id == 2 {
                buildings[0][8] = 1;
                buildings[0][0x1c..0x20].fill(255);
                buildings[0][0x40..0x44].copy_from_slice(&0x617628_u32.to_le_bytes());
            }
            let mut state = State {
                building_parameters: crate::building_parameters::BuildingKinds::from_image(&image)
                    .unwrap(),
                world: business_binding::World {
                    buildings: &mut buildings,
                    businesses: businesses.as_mut_slice(),
                    categories: &mut categories,
                    category_base: 0x617628,
                    catalog: &catalog,
                    available: &mut available,
                    availability_cursor: &mut cursor,
                    players: &[None; 6],
                    resident_count: 0,
                },
                runtime: business_binding::Runtime {
                    building_base: 0x591318,
                    products: &[[[0; 2]; 6]; 45],
                    commodities: &[[0; 0x1c]; 32],
                    residents: &mut [],
                    timestamp: 0,
                },
                image: &image,
                hour: 0,
                discount: 0.,
                date: [0; 2],
                ownership_timestamp: 0,
                utility_owners: [0; 4],
                notifications: false,
                construction_components: [0.; 4],
            };
            let mut vm = Runtime::new().unwrap();
            let bindings = Bindings::register(&mut vm, 1, 2).unwrap();
            let mut hooks = Headless {
                calls: vec![],
                utilities: crate::economy::Utilities::new([0; 4]),
                objective: None,
                prompts: vec![],
                paused: false,
                frequency: crate::timers::CalendarFrequency {
                    frequency: 0.,
                    calendar_timer: None,
                    logic_timer: None,
                },
                timers: vec![[0; 20]; 2],
            };
            let mut observed = None;
            vm.execute("native map callback", &source, &mut |called, args| {
                assert_eq!(called, id);
                if id == 2 {
                    observed = Business::from_arguments(args, &catalog)?;
                }
                bindings.handle(called, args, &mut state, &mut Geometry, &mut hooks)
            })
            .unwrap();
            assert_eq!(
                vm.number("result").unwrap(),
                (returns != 0).then_some(returned),
                "case {case}"
            );
            if let Some(words) = constructor {
                let row = &state.world.buildings[0];
                assert_eq!(word(row, 0x40), words[0]);
                assert_eq!(words[1], 0x591318);
                assert_eq!(
                    row[4..8],
                    words[2..6].iter().map(|&v| v as u8).collect::<Vec<_>>()
                );
                assert_eq!(row[8], words[6] as u8);
                assert_eq!(word(row, 0x20), words[7]);
                assert_eq!(word(row, 0x24), words[8]);
                assert_eq!(row[0x2c], words[9] as u8);
                assert_eq!(word(row, 0x38), words[10]);
            } else if id == 1 {
                assert_eq!(
                    state
                        .world
                        .buildings
                        .iter()
                        .filter(|r| r[0x3f] != 0)
                        .count(),
                    0
                );
            }
            if let Some(request) = observed {
                assert_eq!(
                    native_calls,
                    vec![
                        [0x4634c0, request.kind, 0, request.building as u32],
                        [0x463560, 0, request.building as u32, 1]
                    ]
                );
                assert_eq!(word(&state.world.businesses[0], 4), request.kind);
            } else {
                assert!(native_calls.is_empty());
            }
        }
        assert_eq!(reader.at, reader.data.len());
    }

    #[test]
    #[ignore = "requires original map, configuration and building geometry"]
    fn original_starting_out_hub_creates_city_and_population() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = crate::build_file::BuildFile::parse(&file_bytes).unwrap();
        let map = read("OPENMT2001_MAP");
        let configuration = || {
            crate::resident::Configuration::before_defaults(
                std::str::from_utf8(&read("OPENMT2001_PARAMETERS")).unwrap(),
                std::str::from_utf8(&read("OPENMT2001_TIMETABLE")).unwrap(),
                &image,
            )
            .unwrap()
        };
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = crate::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let people_models =
            crate::people::PeopleModels::load(&image, &directory, &archive).unwrap();
        let handles = std::array::from_fn(|i| [(i * 2 + 1) as u32, (i * 2 + 2) as u32]);
        let model_bindings = people_models.resident_bindings(&handles).unwrap();
        let default_path = crate::fixtures::path("DEFAULT_SETTINGS");
        let scripts = default_path.parent().unwrap().parent().unwrap();
        let prefs = std::fs::read(scripts.join("SCENARIO3/initialpref.lua")).unwrap();
        for (scenario, difficulty) in ["SCENARIO1", "SCENARIO3", "SCENARIO5"]
            .into_iter()
            .flat_map(|scenario| (0..3).map(move |difficulty| (scenario, difficulty)))
        {
            let hub = std::fs::read(scripts.join(scenario).join("hub.lua")).unwrap();
            let scenario_map = if scenario == "SCENARIO1" {
                std::fs::read(scripts.join("MAPS/apartments2.lua")).unwrap()
            } else {
                map.clone()
            };

            let expected =
                crate::scene::parse_static_map(std::str::from_utf8(&scenario_map).unwrap())
                    .unwrap();
            let mut session = crate::level_startup::Scripts::new(&image).unwrap();
            session
                .runtime
                .script_buffer("scripts/maps/apartmentrim.lua", &map)
                .unwrap();
            session
                .runtime
                .script_buffer("scripts/scenario3/initialpref.lua", &prefs)
                .unwrap();
            session
                .runtime
                .script_buffer("scripts/maps/apartments2.lua", &scenario_map)
                .unwrap();
            session
                .runtime
                .script_buffer(
                    "scripts/scenario1/initialpref.lua",
                    &std::fs::read(scripts.join("SCENARIO1/initialpref.lua")).unwrap(),
                )
                .unwrap();
            // Controlled host seed, passed through the original complete static
            // registry instead of independently invented resident stream states.
            let startup_rng = crate::startup_rng::Snapshot::from_seed(0x12345678);
            let creation =
                startup_rng.resident_creation(crate::resident::AuxiliaryPool::for_residents());
            let mut world =
                crate::level_startup::World::fresh(&image, &board, configuration(), creation)
                    .unwrap();
            assert_eq!(
                world.topology.count617624(),
                52,
                "original449a40 count includes inactive and unused topology rows"
            );
            let centers = board.category_geometry().centers;
            // Geometry adapter supplied below; no saved city records are imported.
            struct Models {
                count: u32,
            }
            impl crate::building_geometry::ModelRegistrar for Models {
                fn finish_binding(
                    &mut self,
                    _: &crate::scene::SceneBuilding,
                    _: &mut [u8; 0x394],
                ) -> Result<(), String> {
                    // Explicit headless boundary: no GPU matrix/manager/appearance submission.
                    Ok(())
                }
                fn register(
                    &mut self,
                    scene: &crate::scene::SceneBuilding,
                ) -> Result<crate::building_geometry::RegisteredModel, String> {
                    let prepared = crate::geometry::prepare(&scene.layers);
                    let positions = crate::geometry::facade_positions(&prepared)
                        .into_iter()
                        .chain(crate::roof::mesh(&prepared).positions)
                        .map(|p| scene.local_position(p));
                    let bytes: Vec<_> = positions
                        .flat_map(|p| p.into_iter().flat_map(f32::to_le_bytes))
                        .collect();
                    let definition = crate::mesh::model_definition(&bytes)?;
                    self.count += 1;
                    crate::building_geometry::RegisteredModel::from_definition(
                        self.count,
                        &definition,
                    )
                }
            }
            // Explicit model-registration seam: facade/roof model bounds and local
            // handles, not a complete native graphics-manager registration.
            let mut geometry =
                crate::building_geometry::BuildingGeometry::new(&board, &file, Models { count: 0 });
            let mut hooks = Headless {
                calls: vec![],
                utilities: crate::economy::Utilities::new([0; 4]),
                objective: None,
                prompts: vec![],
                paused: false,
                frequency: crate::timers::CalendarFrequency {
                    frequency: 0.,
                    calendar_timer: None,
                    logic_timer: None,
                },
                timers: vec![[0; 20]; 2],
            };
            let mut context = world
                .script_context(
                    &image,
                    &model_bindings,
                    crate::level_startup::HostContext {
                        accounting_enabled: false,
                        difficulty: difficulty as i32,
                        notifications: false,
                        auxiliary_template: [0xa5; 0x11c],
                        category_base: 0x617628,
                        building_base: 0x591318,
                    },
                )
                .unwrap();
            for name in [
                "auctioncamera",
                "sound",
                "general",
                "commoditysettings",
                "blocksettings",
                "businesssettings",
                "buildingsettings",
                "default",
            ] {
                session
                    .runtime
                    .script_buffer(
                        &format!("scripts/default/{name}.lua"),
                        &std::fs::read(scripts.join(format!("DEFAULT/{name}.lua"))).unwrap(),
                    )
                    .unwrap();
            }
            session
                .initialize(
                    crate::level_startup::Sources {
                        default_hub: &std::fs::read(scripts.join("DEFAULT/hub.lua")).unwrap(),
                        scenario_name: scenario,
                        scenario_hub: &hub,
                    },
                    &mut world.script_state(&[None; 6]),
                    &context,
                    &mut geometry,
                    &mut hooks,
                )
                .unwrap();
            let before_synthetic_total = world.clock.timing[0];
            if scenario == "SCENARIO5" {
                let mut seen = Vec::new();
                for tick in 1..=8125 {
                    world.clock.timing[0] = tick;
                    let before = hooks.prompts.len();
                    session
                        .runtime
                        .tick_events(&mut session.events, &mut |id, args, events| {
                            session.bindings.handle(
                                id,
                                args,
                                &mut world.script_state(&[None; 6]),
                                events,
                                &context,
                                &mut geometry,
                                &mut hooks,
                            )
                        })
                        .unwrap();
                    if hooks.prompts.len() != before {
                        seen.push(tick);
                        assert_eq!(hooks.prompts.len(), before + 1);
                        assert_eq!(
                            hooks.prompts.last().unwrap(),
                            &vec![
                                Value::Number(1934. + before as f64 * 2.),
                                Value::Number(1935. + before as f64 * 2.),
                                Value::Number(1.),
                                Value::Number(0.), // ppnn: omitted final numeric argument
                            ]
                        );
                        assert!(world.pause.has_reason());
                        session
                            .runtime
                            .call("CATCHEVENT_InterfacePromptDismiss", &[], &mut |id, args| {
                                session.bindings.handle(
                                    id,
                                    args,
                                    &mut world.script_state(&[None; 6]),
                                    &mut session.events,
                                    &context,
                                    &mut geometry,
                                    &mut hooks,
                                )
                            })
                            .unwrap();
                        assert!(!world.pause.has_reason());
                    }
                }
                assert_eq!(
                    seen,
                    if difficulty == 0 {
                        vec![2650, 4050, 5000, 6875, 8125]
                    } else {
                        vec![]
                    }
                );
            }
            world.clock.timing[0] = before_synthetic_total;
            // Existing detailed subsystem checks borrow the initialized owned
            // world below; production callers can keep World/Scripts together.
            let mut routes = world.category_routes(&board);
            let crate::level_startup::World {
                mut clock,
                city:
                    crate::level_startup::City {
                        mut objective,
                        mut settings,
                        configuration: mut config,
                        mut welfare,
                        mut pause,
                        mut buildings,
                        ledgers:
                            crate::economy::Ledgers {
                                mut businesses,
                                mut residents,
                                players: _,
                            },
                        mut categories,
                        mut road_network,
                        mut definitions,
                        mut commodities,
                        mut catalog,
                        mut creation,
                        mut available,
                        availability_cursor: mut cursor,
                        player_money: mut cash,
                        mut construction_components,
                        pending_world: _,
                        ownership_refresh_counter: _,
                        auction: _,
                        topology: _,
                        category_footprints: _,
                        ai_neighbors: _,
                        ai_demand: _,
                        building_parameters: _,
                    },
            } = world;
            let crate::level_startup::Scripts {
                runtime: mut vm,
                bindings: level,
                mut events,
            } = session;
            for (value, name) in [
                "OBJECTIVE_NONE",
                "OBJECTIVE_FIRST_TO_BE_MAYOR",
                "OBJECTIVE_WEAKEST_LINK",
                "OBJECTIVE_PROFIT",
                "OBJECTIVE_PAPERWEALTH_BY_DATE",
                "OBJECTIVE_PAPERWEALTH_BY_AMOUNT",
                "OBJECTIVE_SALES",
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(vm.number(name).unwrap(), Some(value as f64));
            }
            assert_eq!(objective.kind, if scenario == "SCENARIO1" { 4 } else { 6 });
            assert_eq!(
                objective.target,
                match scenario {
                    "SCENARIO1" => 1945.,
                    "SCENARIO5" => 200.,
                    _ => 100.,
                }
            );
            // Original489490 initializes flag1; only Scenario3 here supplies
            // an explicit third SetObjective argument to disable it.
            assert_eq!(objective.flag, u8::from(scenario != "SCENARIO3"));
            assert_eq!(settings.num_cars, 125);
            assert_eq!(
                settings.traffic,
                [
                    [15, 10, 40, 40, 30, 20],
                    [20, 40, 20, 10, 10, 10],
                    [15, 5, 5, 10, 10, 25]
                ]
            );
            assert_eq!((settings.itchy_counter, settings.subway_threshold), (2, 5));
            assert_eq!(
                (settings.years_per_day(), settings.lease_length()),
                (5, 120)
            );
            assert!(settings.city_businesses_ownable() && settings.days_off());
            for i in 0..3 {
                assert_eq!(settings.rail_income(i), Some(1.));
                assert_eq!(settings.daily_freight(i), Some(800.));
            }
            assert_eq!(hooks.frequency.calendar_timer, None);
            assert_eq!(hooks.frequency.frequency, 0.);
            let housing =
                crate::housing::Topology::from_runtime(&buildings, &categories, 0x591318).unwrap();
            let shopping_settings =
                crate::shopping::Settings::from_runtime(&catalog, &commodities, &config).unwrap();
            assert_eq!(
                shopping_settings.weights,
                [
                    [0.40, -0.40, 0.01, 1.60],
                    [0.20, 0.01, 0.20, 2.40],
                    [0.10, 0.20, 0.50, 3.60],
                ]
            );
            assert_eq!(shopping_settings.business_distance_weights, [0.01; 3]);
            let source_catalog = crate::shopping::Catalogs::from_v14_defaults(
                std::str::from_utf8(&read("OPENMT2001_BUSINESS_SETTINGS")).unwrap(),
                &image,
            )
            .unwrap();
            assert_eq!(
                shopping_settings.catalogs.commodities,
                source_catalog.commodities
            );
            assert_eq!(
                shopping_settings.catalogs.fractions,
                source_catalog.fractions
            );
            let shopping =
                crate::shopping::Topology::from_runtime(&buildings, &categories, 0x591318, &board)
                    .unwrap();
            assert_eq!(
                shopping.nodes.iter().map(Vec::len).sum::<usize>(),
                expected.len()
            );
            assert_eq!(
                housing
                    .categories
                    .iter()
                    .map(|category| category.buildings.len())
                    .sum::<usize>(),
                expected.len()
            );
            // Verify authored placement and pool links for both map layouts.
            for (slot, spec) in
                crate::scene::parse_static_map(std::str::from_utf8(&scenario_map).unwrap())
                    .unwrap()
                    .into_iter()
                    .enumerate()
            {
                let scene = crate::scene::SceneBuilding::resolve(spec, &board, &file).unwrap();
                let building = &buildings[slot];
                assert_eq!(housing.buildings[slot].business, Some(slot));
                assert_eq!(housing.buildings[slot].category, scene.block_id as i8);
                assert_eq!(shopping.building_categories[slot], scene.block_id as i16);
                assert_eq!(shopping.building_businesses[slot], Some(slot));
                assert_eq!(
                    shopping.entrances[slot],
                    scene.entrance_positions(centers[scene.block_id]).unwrap()[0]
                );
                let node = shopping.nodes[scene.block_id]
                    .iter()
                    .find(|node| node.building == slot as i32)
                    .unwrap();
                assert_eq!(node.position, scene.center);
                assert_eq!(
                    housing.buildings[slot].housing_position,
                    scene.entrance_positions(centers[scene.block_id]).unwrap()[0]
                );

                assert_eq!(word(building, 0x1c), slot as u32);
                assert_eq!(
                    u16::from_le_bytes(businesses[slot][8..10].try_into().unwrap()),
                    slot as u16
                );
                assert_eq!(
                    word(building, 0x40),
                    0x617628 + scene.block_id as u32 * 0x17c
                );
                assert_eq!(
                    [
                        f32::from_bits(word(building, 0xc)),
                        f32::from_bits(word(building, 0x10))
                    ],
                    scene.center
                );
            }
            if scenario == "SCENARIO1" {
                assert_eq!(residents.active_count, [423, 340, 280][difficulty]);
                assert_eq!(cash, [8000., 6500., 5000.][difficulty]);
                assert_eq!(
                    hooks.objective,
                    Some(vec![Value::Number(4.), Value::Number(1945.), Value::Nil])
                );
                assert!(events.pending().is_empty());
                assert_eq!(
                    buildings
                        .iter()
                        .filter(|row| row[0x170..0x174] != [0; 4])
                        .count(),
                    expected.len()
                );
                assert_eq!(
                    businesses.iter().filter(|row| row[0x21c] != 0).count(),
                    expected.len()
                );
                assert_eq!(categories.iter().filter(|row| row[0x74] != 0).count(), 33);
                assert_eq!(categories.iter().filter(|row| row[0x78] == 2).count(), 25);
                // Roster and ongoing objective evaluation are separate boundaries.
                continue;
            }
            if scenario == "SCENARIO5" {
                // The shared startup API also loads this different authored hub.
                // Its AI roster/postludes remain a separate integration boundary;
                // city initialization and tutorial bodies are verified here.
                assert_eq!(residents.active_count, [423, 340, 280][difficulty]);
                assert_eq!(cash, [15000., 12000., 10000.][difficulty]);
                assert_eq!(
                    buildings
                        .iter()
                        .filter(|row| row[0x170..0x174] != [0; 4])
                        .count(),
                    9
                );
                assert_eq!(businesses.iter().filter(|row| row[0x21c] != 0).count(), 9);
                assert_eq!(
                    hooks.objective,
                    Some(vec![Value::Number(6.), Value::Number(200.), Value::Nil])
                );
                assert!(events.pending().is_empty());
            }
            // The actual scenario roster consumes InitialValue.playermoney
            // written by the same level script, and observes the same city.
            let mut roster = crate::player_script::PendingRoster::default();
            roster
                .load_normal(
                    scenario,
                    &std::fs::read(scripts.join(scenario).join("players.lua")).unwrap(),
                    difficulty as i32,
                    4,
                )
                .unwrap();
            let mut retained_roster = crate::level_startup::Roster::new([0xa5; 0x28]).unwrap();
            let mut registry = crate::player_manager::RandomRegistry {
                process: startup_rng.process,
                head: crate::startup_rng::HEAD,
                tail: crate::startup_rng::TAIL,
            };
            let mut static_tail_next = 0;
            let mut reset = 0;
            let mut next = 0x10010000_u32;
            let mut allocator = |size: usize| {
                let reference = next;
                next += (size as u32 + 0xfff) & !0xfff;
                Ok(Some(crate::player_constructor::Allocation {
                    reference,
                    bytes: vec![0xa5; size],
                }))
            };
            let chains =
                crate::building_pool::category_chains(&categories, &buildings, 0x591318).unwrap();
            assert_eq!(chains.iter().map(Vec::len).sum::<usize>(), 9);
            let mapping = crate::ai_ownership::group_mapping(&image).unwrap();
            let business_categories = std::array::from_fn(|slot| {
                if businesses[slot][0x21c] == 0 {
                    return -1;
                }
                let building =
                    u16::from_le_bytes(businesses[slot][8..10].try_into().unwrap()) as usize;
                ((word(&buildings[building], 0x40) - 0x617628) / 0x17c) as i16
            });
            let scenario_ai = std::fs::read(scripts.join(scenario).join("ai.lua")).ok();
            let mut ownership_passes = 0;
            retained_roster
                .initialize(
                    &roster,
                    crate::player_script::RosterInputs {
                        names: [b"Fixture Player"; 6],
                        initial_cash: cash,
                        default_ai: &read("OPENMT2001_AI_SCRIPT"),
                        scenario_ai: scenario_ai
                            .as_deref()
                            .map(|bytes| ("scenario/ai.lua", bytes)),
                    },
                    &mut crate::player_constructor::RosterConstruction {
                        image: &image,
                        registry: &mut registry,
                        global_reset: &mut reset,
                        allocate: &mut allocator,
                        external_rng_link: &mut |reference, next| {
                            assert_eq!(reference, crate::startup_rng::TAIL);
                            assert_eq!(static_tail_next, 0);
                            static_tail_next = next;
                            Ok(())
                        },
                    },
                    crate::level_startup::RosterCity {
                        categories: &categories,
                        buildings: &buildings,
                        businesses: businesses.as_slice(),
                        clock: &clock,
                        difficulty: difficulty as i32,
                        building_base: 0x591318,
                    },
                    &mut |manager, reference| {
                        let player = manager
                            .players
                            .iter_mut()
                            .find(|p| p.reference == reference)
                            .unwrap();
                        let mut ownership = crate::ai_ownership::State {
                            completed_groups: player.bytes[0xaa8],
                            groups: std::array::from_fn(|i| {
                                player.bytes[0xaac + i * 12..0xaac + (i + 1) * 12]
                                    .try_into()
                                    .unwrap()
                            }),
                            lists: std::array::from_fn(|_| crate::ai_ownership::BusinessList {
                                entries: vec![],
                                active_count: 0,
                            }),
                        };
                        ownership.maintain(
                            player.bytes[0x1bc],
                            player.bytes[0x178..0x1ac].try_into().unwrap(),
                            &mapping,
                            businesses.as_slice(),
                            &business_categories,
                            |_| Err("new AI unexpectedly owns an initialized city business".into()),
                        )?;
                        assert!(ownership.lists.iter().all(|list| list.entries.is_empty()));
                        player.bytes[0xaa8] = ownership.completed_groups;
                        for (i, group) in ownership.groups.iter().enumerate() {
                            player.bytes[0xaac + i * 12..0xaac + (i + 1) * 12]
                                .copy_from_slice(group);
                        }
                        ownership_passes += 1;
                        Ok(())
                    },
                )
                .unwrap();
            let manager = &mut retained_roster.manager;
            let status = &retained_roster.status;
            assert_ne!(static_tail_next, 0);
            assert_eq!(registry.head, crate::startup_rng::HEAD);
            let ai_count = usize::from(scenario == "SCENARIO5");
            assert_eq!(manager.players.len(), 1 + ai_count);
            assert_eq!(status.ai_count as usize, ai_count);
            assert_eq!(ownership_passes, ai_count);
            assert_eq!(status.has_ai, ai_count != 0);
            assert_eq!(status.current_player, manager.players[0].reference);
            assert_eq!(
                crate::economy::PlayerAccount::from_runtime(&manager.players[0].bytes)
                    .unwrap()
                    .cash,
                cash
            );
            assert_eq!(word(&manager.players[0].bytes, 0x150), 4);
            if scenario == "SCENARIO5" {
                let opponent = &manager.players[1];
                assert_eq!(word(&opponent.bytes, 0x150), 6); // IRON
                assert_eq!(word(&opponent.bytes, 0x13f4), [0, 0, 1][difficulty]);
                assert_eq!(
                    crate::economy::PlayerAccount::from_runtime(&opponent.bytes)
                        .unwrap()
                        .cash,
                    cash
                );
                assert_eq!(residents.active_count, [423, 340, 280][difficulty]);
                assert_eq!(businesses.iter().filter(|row| row[0x21c] != 0).count(), 9);
                continue;
            }
            // Exercise the recovered late category propagation after roster
            // creation; other post-roster managers are still outside this test.
            crate::category_reset::refresh_prestige(&mut categories, &definitions.reset_values());
            for (row, value) in categories.iter().zip(definitions.reset_values()) {
                assert_eq!(word(row, 0x84), value);
            }
            // Remaining direct map calls below only exercise refused placement.
            let bindings = Bindings::register(&mut vm, 1, 2).unwrap();
            let mut state = State {
                building_parameters: crate::building_parameters::BuildingKinds::from_image(&image)
                    .unwrap(),
                world: business_binding::World {
                    buildings: &mut buildings,
                    businesses: businesses.as_mut_slice(),
                    categories: &mut categories,
                    category_base: 0x617628,
                    catalog: &catalog.info,
                    available: &mut available,
                    availability_cursor: &mut cursor,
                    players: &[None; 6],
                    resident_count: 0,
                },
                runtime: business_binding::Runtime {
                    building_base: 0x591318,
                    products: &catalog.products,
                    commodities: &commodities.rows,
                    residents: &mut [],
                    timestamp: 0,
                },
                image: &image,
                hour: clock.hour() as i32,
                discount: 0.5,
                date: [0; 2],
                ownership_timestamp: 0,
                utility_owners: [0; 4],
                notifications: false,
                construction_components,
            };
            assert_eq!(hooks.calls.iter().filter(|c| c.0 == 0x462070).count(), 9);
            assert_eq!(hooks.calls.iter().filter(|c| c.0 == 0x48dfc0).count(), 9);
            assert_eq!(hooks.utilities.totals, [0.; 4]);
            let count = [423, 340, 280][difficulty];
            assert_eq!(residents.active_count, count);
            assert_eq!(creation.auxiliary.used(), count as usize);
            assert_eq!(
                state
                    .world
                    .categories
                    .iter()
                    .map(|r| word(r, 0x88))
                    .sum::<u32>(),
                count
            );
            assert_eq!(cash, [15000., 12000., 10000.][difficulty]);
            assert_eq!(
                hooks.objective,
                Some(vec![
                    Value::Number(6.),
                    Value::Number(100.),
                    Value::Number(0.)
                ])
            );
            assert!(hooks.prompts.is_empty());
            assert_eq!(events.pending().len(), if difficulty == 0 { 9 } else { 0 });
            if difficulty == 0 {
                assert_eq!(
                    events
                        .pending()
                        .iter()
                        .map(|e| (e.argument, e.remaining))
                        .collect::<Vec<_>>(),
                    vec![
                        (1, 300),
                        (2, 1250),
                        (3, 2600),
                        (4, 4425),
                        (5, 5775),
                        (6, 8750),
                        (7, 9375),
                        (8, 10625),
                        (9, 13125)
                    ]
                );
            }
            assert!(state.world.categories.iter().all(|r| r[0x74] == 0));
            assert_eq!(
                state
                    .world
                    .categories
                    .iter()
                    .filter(|r| r[0x78] == 2)
                    .count(),
                25
            );
            for resident in &residents.nodes[..count as usize] {
                assert!(resident.is_active());
                assert_eq!(resident.housing_building(), -1);
            }

            assert_eq!(
                state.world.available.as_slice(),
                &(0..9).collect::<Vec<_>>()
            );
            assert_eq!(*state.world.availability_cursor, Some(0));
            assert_eq!(
                state
                    .world
                    .buildings
                    .iter()
                    .filter(|r| r[0x3f] != 0)
                    .count(),
                9
            );
            assert_eq!(
                state
                    .world
                    .businesses
                    .iter()
                    .filter(|r| r[0x21c] != 0)
                    .count(),
                9
            );
            for (slot, spec) in expected.into_iter().enumerate() {
                let scene = crate::scene::SceneBuilding::resolve(spec, &board, &file).unwrap();
                let building = &state.world.buildings[slot];
                let business = &state.world.businesses[slot];
                assert_eq!(word(building, 0x1c), slot as u32);
                assert_eq!(
                    u16::from_le_bytes(business[8..10].try_into().unwrap()),
                    slot as u16
                );
                assert_eq!(word(business, 4), 42);
                assert_eq!(word(building, 0x14), 4);
                assert_eq!(
                    word(building, 0x40),
                    0x617628 + (scene.block_id as u32) * 0x17c
                );
                assert_eq!(
                    [
                        f32::from_bits(word(building, 0xc)),
                        f32::from_bits(word(building, 0x10))
                    ],
                    scene.center
                );
                let capacity = building_pool::capacity(
                    building,
                    f32::from_bits(word(catalog.info.row(42).unwrap(), 0x34)),
                ) as u16;
                assert_eq!(
                    u16::from_le_bytes(business[0xe..0x10].try_into().unwrap()),
                    capacity
                );
                assert!(capacity > 0);
                let positions = scene.entrance_positions(centers[scene.block_id]).unwrap();
                assert_eq!(word(building, 0x220) as usize, positions.len());
                for (i, p) in positions.into_iter().enumerate() {
                    for (axis, v) in p.into_iter().enumerate() {
                        assert_eq!(word(building, 0x224 + i * 12 + axis * 4), v.to_bits());
                    }
                }
            }
            // Original Lua distinguishes unknown names (no result) from refused
            // occupancy (numeric -1); refusal must not allocate another slot.
            vm.execute("map refusal",b"unknown=AddBuilding('NO_SUCH_BLOCK',0,0,3,3,8,'RESIDENTIAL',176,'high'); refused=AddBuilding('INDIANA_AVENUE',0,0,3,3,8,'RESIDENTIAL',176,'high'); AddBusiness(refused,'apartments')",&mut |id,args|bindings.handle(id,args,&mut state,&mut geometry,&mut hooks)).unwrap();
            let snapshot = (
                state.world.buildings.to_vec(),
                state.world.businesses.to_vec(),
                *state.world.categories,
                state.construction_components,
            );
            let calls = hooks.calls.len();
            vm.execute("repeat apartmentrim", &map, &mut |id, args| {
                bindings.handle(id, args, &mut state, &mut geometry, &mut hooks)
            })
            .unwrap();
            assert_eq!(state.world.buildings, snapshot.0);
            assert_eq!(state.world.businesses, snapshot.1);
            assert_eq!(*state.world.categories, snapshot.2);
            assert_eq!(state.construction_components, snapshot.3);
            assert_eq!(hooks.calls.len(), calls);
            assert_eq!(vm.number("unknown").unwrap(), None);
            assert_eq!(vm.number("refused").unwrap(), Some(-1.));
            assert_eq!(
                state
                    .world
                    .businesses
                    .iter()
                    .filter(|r| r[0x21c] != 0)
                    .count(),
                9
            );
            // Keep the actual initialization event list for the combined run.
            // The independent tutorial-body probe uses its own VM references.
            let simulation_events = std::mem::take(&mut events);
            for event in simulation_events.pending() {
                events.append(crate::script_events::Event {
                    reference: vm
                        .reference_function("EVENT_CreatePrompt")
                        .unwrap()
                        .unwrap(),
                    remaining: event.remaining,
                    argument: event.argument,
                });
            }
            // Preserve the original startup pools before controlled script/housing probes.
            let initial_nodes = residents.nodes.clone();
            let initial_accounts = residents.accounts.clone();
            let initial_buildings = buildings.clone();
            let initial_categories = categories;
            let initial_businesses = businesses.to_vec();
            let initial_available = available.clone();
            let initial_auxiliary = crate::resident::AuxiliaryPool::from_slots(
                creation.auxiliary.slots().to_vec(),
                creation.auxiliary.used(),
            )
            .unwrap();
            let initial_constructor_rng = creation.initialization_rng.constructor;
            let initial_objective_rng = creation.initialization_rng.objective;
            // A subsequent scripted addition must invalidate the actual
            // residents, not an empty pre-population callback view.
            let category = ((word(&buildings[0], 0x40) - context.category_base) / 0x17c) as usize;
            let byte_at = 0x2c + category / 4;
            let shift = (category % 4) * 2;
            let before: Vec<_> = residents.nodes[..count as usize]
                .iter()
                .map(|r| (r.as_bytes()[byte_at] >> shift) & 3)
                .collect();
            assert!(before.iter().any(|&value| value < 3));
            let before_synthetic_total = clock.timing[0];
            {
                let mut state = crate::level_script::State {
                    clock: &mut clock,
                    ai_demand: &mut crate::ai_demand::State::default(),
                    objective: &mut objective,
                    settings: &mut settings,
                    configuration: &mut config,
                    welfare: &mut welfare,
                    pause: &mut pause,
                    buildings: &mut buildings,
                    businesses: businesses.as_mut_slice(),
                    categories: &mut categories,
                    definitions: &mut definitions,
                    commodities: &mut commodities,
                    catalog: &mut catalog,
                    residents: &mut residents,
                    creation: &mut creation,
                    available: &mut available,
                    availability_cursor: &mut cursor,
                    players: &[None; 6],
                    player_accounts: &mut [],
                    player_money: &mut cash,
                    construction_components: &mut construction_components,
                };
                hooks.frequency.calendar_timer = Some(0);
                hooks.frequency.logic_timer = Some(1);
                vm.execute("calendar frequency", b"GAME_HOUR=1; Calendar.SetFrequency(99, '0.04'); calendar_result=GAME_HOUR; decade_result=GAME_DECADE", &mut |id,args| {
                    level.handle(id,args,&mut state,&mut events,&context,&mut geometry,&mut hooks)
                }).unwrap();
                assert_eq!(vm.number("calendar_result").unwrap(), Some(625.));
                assert_eq!(vm.number("decade_result").unwrap(), Some(150000.));
                assert_eq!(vm.number("GAME_DAY").unwrap(), None);
                assert_eq!(hooks.frequency.frequency.to_bits(), 0.04_f32.to_bits());
                assert_eq!(f32::from_bits(word(&hooks.timers[1], 8)), 40.);
                let ownership_tick = state.clock.timing[1];
                state.clock.timing[1] = 0x11223344;
                // One actual scheduled callback invokes a nested buffer. Native map
                // construction must consume that buffer's immediately written date.
                vm.script_buffer("calendar-date-inner.lua",b"Calendar.era=5; Calendar.year=7; Calendar.hour=13; extra=AddBuilding('INDIANA_AVENUE',4,4,3,3,8,'RESIDENTIAL',176,'high'); AddBusiness(extra,'apartments')").unwrap();
                vm.execute("scheduled calendar consumer", b"function CalendarDateConsumer() old_era=Calendar.era; old_year=Calendar.year; old_hour=Calendar.hour; C_RunScript('calendar-date-inner.lua'); Calendar.era=old_era; Calendar.year=old_year; Calendar.hour=old_hour; return 0 end", &mut |id,args|level.handle(id,args,&mut state,&mut events,&context,&mut geometry,&mut hooks)).unwrap();
                let mut calendar_events = crate::script_events::Events::default();
                calendar_events.append(crate::script_events::Event {
                    reference: vm
                        .reference_function("CalendarDateConsumer")
                        .unwrap()
                        .unwrap(),
                    remaining: 1,
                    argument: 0,
                });
                vm.tick_events(&mut calendar_events, &mut |id, args, events| {
                    level.handle(
                        id,
                        args,
                        &mut state,
                        events,
                        &context,
                        &mut geometry,
                        &mut hooks,
                    )
                })
                .unwrap();
                assert!(calendar_events.pending().is_empty());
                state.clock.timing[1] = ownership_tick;
                let extra = vm
                    .number("extra")
                    .unwrap()
                    .expect("actual map construction") as usize;
                assert_eq!(
                    state.buildings[extra][0x34], 57,
                    "same-chunk native construction date"
                );
                assert_eq!(
                    word(&state.buildings[extra], 0x30),
                    0x11223344,
                    "actual ownership-domain timestamp"
                );
                // Exercise the original tutorial event bodies and dismissal
                // handler through the production bindings. This advances only
                // script ticks, not a claim of full-city simulation/gameplay.
                let due = [300, 1250, 2600, 4425, 5775, 8750, 9375, 10625, 13125];
                let mut seen = vec![];
                for tick in 1..=13125 {
                    state.clock.timing[0] = tick;
                    let previous = hooks.prompts.len();
                    vm.tick_events(&mut events, &mut |id, args, events| {
                        level.handle(
                            id,
                            args,
                            &mut state,
                            events,
                            &context,
                            &mut geometry,
                            &mut hooks,
                        )
                    })
                    .unwrap();
                    if hooks.prompts.len() != previous {
                        assert_eq!(hooks.prompts.len(), previous + 1);
                        assert!(hooks.paused);
                        assert!(state.pause.has_reason());
                        seen.push(tick);
                        assert_eq!(
                            hooks.prompts.last().unwrap(),
                            &vec![
                                Value::Number(1898. + previous as f64 * 2.),
                                Value::Number(1899. + previous as f64 * 2.),
                                Value::Number(1.),
                                Value::Number(0.), // ppnn: omitted final numeric argument
                            ]
                        );
                        vm.call("CATCHEVENT_InterfacePromptDismiss", &[], &mut |id, args| {
                            level.handle(
                                id,
                                args,
                                &mut state,
                                &mut events,
                                &context,
                                &mut geometry,
                                &mut hooks,
                            )
                        })
                        .unwrap();
                        assert!(!hooks.paused);
                        assert!(!state.pause.has_reason());
                    }
                }
                assert_eq!(
                    seen.as_slice(),
                    if difficulty == 0 { &due[..] } else { &[] }
                );
                assert!(events.pending().is_empty());
                vm.execute(
                    "nested pause reasons",
                    b"Game.Pause(); Game.Pause(); Game.Unpause()",
                    &mut |id, args| {
                        level.handle(
                            id,
                            args,
                            &mut state,
                            &mut events,
                            &context,
                            &mut geometry,
                            &mut hooks,
                        )
                    },
                )
                .unwrap();
                assert_eq!(state.pause.bytes[0], 1);
                assert!(hooks.paused);
                vm.execute(
                    "last pause release",
                    b"Game.Unpause(); Game.Unpause()",
                    &mut |id, args| {
                        level.handle(
                            id,
                            args,
                            &mut state,
                            &mut events,
                            &context,
                            &mut geometry,
                            &mut hooks,
                        )
                    },
                )
                .unwrap();
                assert!(!state.pause.has_reason());
                assert!(!hooks.paused);
            }
            clock.timing[0] = before_synthetic_total;
            assert_eq!(vm.number("extra").unwrap(), Some(9.));
            assert_eq!(residents.active_count, count);
            assert_eq!(available.len(), 10);
            for (resident, value) in residents.nodes[..count as usize].iter().zip(before) {
                assert_eq!(
                    (resident.as_bytes()[byte_at] >> shift) & 3,
                    (value + 1).min(3)
                );
            }
            // Exercise the move-in action branch explicitly; this is not an
            // assertion that the scheduler has reached it during startup.
            let topology =
                crate::housing::Topology::from_runtime(&buildings, &categories, 0x591318).unwrap();
            let housing_settings = crate::housing::Settings::from_parameters(
                std::str::from_utf8(&read("OPENMT2001_PARAMETERS")).unwrap(),
                &image,
            )
            .unwrap();
            let values = std::array::from_fn(|i| f32::from_bits(word(&categories[i], 0x84)));
            let mut average = |businesses: &[[u8; 0x220]]| topology.average_rent(businesses);
            let mut housing_world = crate::housing::World {
                demand: None,
                buildings: &topology.buildings,
                categories: &topology.categories,
                category_values: &values,
                businesses: businesses.as_mut_slice(),
                available: &mut available,
                players: &mut [],
                welfare: &welfare,
                settings: &housing_settings,
                average_rent: &mut average,
            };
            assert!((0..count as usize).all(|index| {
                crate::housing::choose_category(&mut residents, index, &mut housing_world)
                    .unwrap()
                    .is_none()
            }));
            // Controlled affordable terms exercise assignment independently of
            // the original unaffordable initial rents. No product override.
            // Pick residents whose actual timetable requests move-in at this
            // hour; their pool indices depend on the original RNG registry.
            let movers: Vec<_> = residents.nodes[..count as usize]
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    config
                        .timetables
                        .action(r.as_bytes()[0x40] as usize, clock.hour() as usize)
                        == Some(5)
                })
                .map(|(i, _)| i)
                .take(2)
                .collect();
            assert_eq!(movers.len(), 2);
            let refused = movers[0];
            let tenant = movers[1];
            let centers = std::array::from_fn(|i| topology.categories[i].center.unwrap());
            // Run the real resident tick gates and timetable refresh before
            // housing dispatch; retain the resident in its shared pool.
            for (index, affordable, expected_done) in [
                (refused, false, false),
                (tenant, true, false),
                (tenant, true, true),
            ] {
                if affordable {
                    housing_world.businesses[0][0x1dc..0x1e0]
                        .copy_from_slice(&10_f32.to_le_bytes());
                }
                residents
                    .tick_resident(
                        index,
                        creation.auxiliary.get_mut(index).unwrap(),
                        &centers,
                        |event, index, residents, auxiliary| match event {
                            crate::resident::TickEvent::Movement => {
                                Ok(residents.nodes[index].advance_movement(auxiliary))
                            }
                            crate::resident::TickEvent::RefreshIntent => {
                                residents.nodes[index].refresh_intent(
                                    &config.timetables,
                                    clock.hour() as usize,
                                    commodities.memberships,
                                )?;
                                Ok(false)
                            }
                            crate::resident::TickEvent::Action => {
                                crate::housing::advance_dispatched_action(
                                    residents,
                                    index,
                                    auxiliary,
                                    &mut housing_world,
                                    &config.objectives,
                                    &mut creation.initialization_rng.objective,
                                    |_, _, _| panic!("unexpected travel during chooser/assignment"),
                                )?;
                                Ok(false)
                            }
                        },
                    )
                    .unwrap();
                if !affordable {
                    assert_eq!(residents.nodes[index].housing_building(), -1);
                    assert_eq!(word(residents.nodes[index].as_bytes(), 0xcc), 12);
                    assert_eq!(word(residents.nodes[index].as_bytes(), 0xd0), 0);
                } else if !expected_done {
                    assert_eq!(residents.nodes[tenant].housing_building(), -1);
                    assert_eq!(word(residents.nodes[tenant].as_bytes(), 0xd0), 7);
                } else {
                    assert_eq!(word(residents.nodes[tenant].as_bytes(), 0xcc), 1);
                }
            }
            let home = residents.nodes[tenant].housing_building() as usize;
            let business = topology.buildings[home].business.unwrap();
            assert_eq!(
                u16::from_le_bytes(
                    housing_world.businesses[business][0x10..0x12]
                        .try_into()
                        .unwrap()
                ),
                1
            );
            assert_eq!(
                word(&housing_world.businesses[business], 0x20c),
                tenant as u32
            );
            assert_eq!(residents.nodes[tenant].next_in_housing(), -1);
            assert!(
                residents.nodes[..count as usize]
                    .iter()
                    .enumerate()
                    .all(|(i, resident)| i == tenant || resident.housing_building() == -1)
            );
            // Restore the untouched startup state after the controlled housing
            // probes. The scheduler below starts from the original city pools.
            residents.nodes = initial_nodes;
            residents.accounts = initial_accounts;
            buildings = initial_buildings;
            categories = initial_categories;
            available = initial_available;
            creation.auxiliary.replace_with(initial_auxiliary);
            creation.initialization_rng.constructor = initial_constructor_rng;
            creation.initialization_rng.objective = initial_objective_rng;
            let topology =
                crate::housing::Topology::from_runtime(&buildings, &categories, 0x591318).unwrap();
            let mut shopping =
                crate::shopping::Topology::from_runtime(&buildings, &categories, 0x591318, &board)
                    .unwrap();
            let shopping_settings =
                crate::shopping::Settings::from_runtime(&catalog, &commodities, &config).unwrap();
            let bindings: Vec<_> = residents.nodes[..count as usize]
                .iter()
                .enumerate()
                .map(|(index, node)| crate::shopping::ResidentBinding {
                    resident_reference: 0x6fd7e8 + index as u32 * 0x150,
                    auxiliary_reference: node.auxiliary_reference(),
                    auxiliary_slot: index,
                    model_frame_count: {
                        let reference = word(&creation.auxiliary.slots()[index], 0);
                        let row = model_bindings
                            .iter()
                            .position(|model| model.reference == reference)
                            .unwrap();
                        u16::try_from(people_models.geometry_source(row, false).unwrap().frames)
                            .unwrap()
                    },
                })
                .collect();
            let mut ledgers = crate::economy::Ledgers {
                businesses: initial_businesses,
                residents,
                players: manager
                    .players
                    .iter()
                    .map(|player| {
                        crate::economy::PlayerAccount::from_runtime(&player.bytes).unwrap()
                    })
                    .collect(),
            };
            let daily_settings = crate::economy::BusinessDaySettings::from_catalog(&catalog.info);
            let building_prefixes: Vec<[u8; 64]> = buildings
                .iter()
                .map(|row| row[..64].try_into().unwrap())
                .collect();
            let owner_bytes = categories.map(|row| row[5]);
            let day_inputs = crate::economy::BusinessDayInputs {
                buildings: &building_prefixes,
                building_categories: &shopping.building_categories,
                category_owners: &owner_bytes,
                catalogs: &shopping_settings.catalogs,
                unit_prices: &shopping_settings.unit_costs,
                settings: &daily_settings,
            };
            let daily_parameters = day_inputs.parameters(&ledgers.businesses).unwrap();
            assert_eq!(
                daily_parameters.iter().filter(|row| row.is_some()).count(),
                ledgers
                    .businesses
                    .iter()
                    .filter(|row| row[0x21c] != 0)
                    .count()
            );
            let mut vehicle_models =
                crate::traffic::road::models::Models::from_original(&image, &directory, &archive)
                    .unwrap();
            let route_bytes = read("OPENMT2001_ROAD_ROUTE_TABLE");
            let road_routes = crate::traffic::road::routes::RoadRoutes::from_bytes(
                &route_bytes,
                road_network.road_segment_count as u32,
            )
            .unwrap();
            let mut road_traffic = crate::traffic::road::runtime::RoadTraffic::fresh(
                &image,
                road_network.entry_count,
                [startup_rng.state(0x6744c0), 0x343fd, 0x269ec3],
                &mut [0; 284],
                0x10020000,
                &mut vehicle_models,
            )
            .unwrap();
            let mut animations = crate::resident::ResidentAnimationPool::default();
            let animation_settings =
                crate::resident::ResidentAnimationSettings::from_image(&image).unwrap();
            let enabled = categories.map(|row| row[4]);
            let mut rng = [creation.initialization_rng.constructor, 0x343fd, 0x269ec3];
            let mut properties = crate::economy::PropertyIncome {
                owners: [0; 4],
                totals: [0.; 4],
            };
            let positions_before: Vec<_> = creation.auxiliary.slots()[..count as usize]
                .iter()
                .map(|row| row[4..16].to_vec())
                .collect();
            let mut saw_animation = false;
            let mut simulation_clock = clock;
            // Explicit normal-mode harness; non-population scheduler helpers
            // are recorded boundaries, not asserted as implemented here.
            simulation_clock.timing[5] = 1;
            let mut controls = crate::scheduler::Controls {
                iterations: 1,
                abort: false,
                alternate_script_active: false,
                network_enabled: false,
                network_override: false,
                deadline: u32::MAX,
                gate_32: false,
                gate_32_offset: false,
                before_six_sound: 0,
            };
            let mut helper_boundaries = std::collections::BTreeSet::new();
            let mut dispatches = [0; 2];
            let mut hourly_session = crate::level_startup::Scripts {
                runtime: vm,
                bindings: level,
                events: simulation_events,
            };
            let account_reference = word(&manager.players[0].bytes, 0x21c);
            let account_before = word(
                &manager.players[0]
                    .auxiliaries
                    .iter()
                    .find(|a| a.reference == account_reference)
                    .unwrap()
                    .bytes,
                0,
            );
            let mut cash_controls = crate::player_manager::CashControls {
                network_enabled: false,
                network_override: false,
            };
            let mut hourly_events = Vec::new();
            let mut hourly_scripts = Vec::new();
            let mut intent_refreshes = Vec::new();
            let mut clock_controls = crate::clock::Controls {
                years_per_day: settings.years_per_day(),
                suppress_rollover_query: false,
                rollover_events_enabled: true,
                autosave_disabled: true,
            };
            let mut hourly_controls = crate::hourly::Controls {
                suppressed: false,
                world_mode: 1,
                before_six_sound: 0,
                network_enabled: false,
                network_override: false,
            };
            hooks.prompts.clear();
            hooks.timers = vec![[0; 20]];
            hooks.timers[0][4..8].copy_from_slice(&2_u32.to_le_bytes());
            crate::timers::configure(&mut hooks.timers[0], 25., 0x481680);
            hooks.frequency.calendar_timer = None;
            hooks.frequency.logic_timer = Some(0);
            let mut timed_tasks = std::mem::take(&mut hooks.timers);
            let mut frame = crate::timers::Frame::default();
            frame.initialize(0);
            let mut host_frames = 0;
            let mut paused_frames = 0;
            let mut prompt_ticks = Vec::new();
            while simulation_clock.timing[0] < 1251 {
                host_frames += 1;
                assert!(
                    host_frames <= 1257,
                    "simulation did not resume after prompt dismissal"
                );
                let tick = simulation_clock.timing[0];
                let countdowns_before: Vec<_> = ledgers.residents.nodes[..count as usize]
                    .iter()
                    .map(|node| word(node.as_bytes(), 0xc0))
                    .collect();
                let refresh_start = intent_refreshes.len();
                crate::scheduler::advance_timed(
                    Some(host_frames * 40),
                    &mut frame,
                    &mut timed_tasks,
                    &mut simulation_clock,
                    &mut controls,
                    |timed_event, clock, _, tasks| {
                        // Move the sole timer vector into host hooks for the callback;
                        // no shadow copy can miss Game.Pause's synchronous writes.
                        std::mem::swap(tasks, &mut hooks.timers);
                        let result = (|| {
                        let crate::scheduler::TimedEvent::Logic(event) = timed_event else {
                            return Err("unexpected auxiliary timer in population fixture".into());
                        };
                        let mut population = crate::shopping::Population {
                            topology: &mut shopping,
                            settings: &shopping_settings,
                            routes: &mut routes,
                            lanes: &mut road_network.lanes,
                            auxiliary: &mut creation.auxiliary,
                            animations: &mut animations,
                            animation_settings: &animation_settings,
                            bindings: &bindings,
                        };
                        match event {
                            crate::scheduler::Event::Residents => {
                                dispatches[0] += 1;
                                population
                                    .tick_with_city_records(
                                        crate::shopping::PopulationRecords {
                                            residents: &mut ledgers.residents,
                                            businesses: &mut ledgers.businesses,
                                            players: &mut ledgers.players,
                                        },
                                        crate::shopping::PopulationTick {
                                            tick: clock.timing[0],
                                            boundary: config.movement_boundary,
                                            distance_disabled: false,
                                        },
                                        &mut crate::shopping::CityActions {
                    demand: None,
                                            housing: &topology,
                                            housing_settings: &housing_settings,
                                            welfare: &welfare,
                                            objectives: &config.objectives,
                                            objective_rng: &mut creation
                                                .initialization_rng
                                                .objective,
                                            constructor_rng: &mut rng,
                                            available: &mut available,
                                            category_enabled: &enabled,
                                            properties: &mut properties,
                                        },
                                        |event_index, event, resident, _| {
                                            assert!(event_index < count as usize);
                                            assert_eq!(
                                                event,
                                                crate::resident::TickEvent::RefreshIntent
                                            );
                                            intent_refreshes.push((clock.timing[0], event_index, clock.hour()));
                                            resident.refresh_intent(
                                                &config.timetables,
                                                clock.hour() as usize,
                                                commodities.memberships,
                                            )?;
                                            Ok(false)
                                        },
                                        |_, helper, _, _| {
                                            Err(format!(
                                                "unexpected initial action helper {helper:?}"
                                            ))
                                        },
                                    )
                                    .unwrap();
                                population.topology.write_resident_counts(&mut categories);
                            }
                            crate::scheduler::Event::Animations => {
                                dispatches[1] += 1;
                                population
                                    .advance_animations(&mut ledgers.residents, |_, _, _| {
                                        Err("unexpected nonresident animation".into())
                                    })?;
                            }
                            crate::scheduler::Event::AdvanceHour => {
                                clock.advance_hour(&mut clock_controls, |event, clock, _| {
                                    assert_eq!(event, crate::clock::Event::Hourly);
                                    let counts = std::array::from_fn(|i| shopping_settings.catalogs.commodities[i].len() as u8);
                                    crate::hourly::advance_ledgers(clock, &mut hourly_controls, &mut ledgers, &counts,
                                        |event, clock, _, ledgers| {
                                            hourly_events.push(event);
                                            // Reconcile the typed accounting view before original
                                            // manager/controller code reads the retained player.
                                            for (player, account) in manager.players.iter_mut().zip(&ledgers.players) {
                                                account.write_runtime(&mut player.bytes)?;
                                            }
                                            match event {
                                                crate::hourly::Event::Helper(0x4895b0) => {
                                                    manager.update_cash_status(clock.hour(), &mut objective.cash_statuses, &mut cash_controls, |event, _, _, _| Err(format!("unexpected first-hour cash event {event:?}")))?;
                                                }
                                                crate::hourly::Event::Script { name, argument } => {
                                                    let bytes = image.at(name, 32)?;
                                                    let end = bytes.iter().position(|&b| b == 0).ok_or("unterminated hourly event name")?;
                                                    let name = std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid hourly event name")?;
                                                    context.update_clock(clock);
                                                    // Optional event handlers are resolved in the retained
                                                    // original scenario VM; no synthetic script body.
                                                    {
                                                        let mut state = crate::level_script::State {
                            clock: clock,
                    ai_demand: &mut crate::ai_demand::State::default(),
                                                            objective: &mut objective, settings: &mut settings,
                                                            configuration: &mut config, welfare: &mut welfare,
                                                            pause: &mut pause, buildings: &mut buildings,
                                                            businesses: &mut ledgers.businesses, categories: &mut categories,
                                                            definitions: &mut definitions, commodities: &mut commodities,
                                                            catalog: &mut catalog, residents: &mut ledgers.residents,
                                                            creation: &mut creation, available: &mut available,
                                                            availability_cursor: &mut cursor, players: &[None; 6],
                    player_accounts: &mut [],
                                                            player_money: &mut cash, construction_components: &mut construction_components,
                                                        };
                                                        let result=hourly_session.dispatch_catch_event((name, argument as i32), &mut state, &context, &mut geometry, &mut hooks);
                                                        context.update_clock(state.clock);
                                                        if result?.is_some() {
                                                            hourly_scripts.push(name.to_owned());
                                                        }
                                                    }
                                                }
                                                crate::hourly::Event::Categories => {
                                                    crate::category::advance_hour(&mut shopping.categories,
                                                        &mut crate::category::Controls {
                                                            current_owner_index: manager.players[0].bytes[0x1bc],
                                                            selected_category: None, selection_active: false,
                                                        }, |call, _, _| Err(format!("unexpected fresh-city expiry {call:?}")))?;
                                                    shopping.write_category_ownership(&mut categories);
                                                }
                                                crate::hourly::Event::CurrentPlayerUpdate => {
                                                    let count = manager.players.len() as i32;
                                                    let player = manager.players.iter_mut().find(|p| p.reference == status.current_player).ok_or("current hourly player is unresolved")?;
                                                    if let Some(command) = player.advance_hourly_account(count)? {
                                                        return Err(format!("unexpected initial hourly command {command:?}"));
                                                    }
                                                }
                                                // Explicit diagnostic boundaries: objective query,
                                                // traffic demand, appearance and UI helpers.
                                                crate::hourly::Event::Query |
                                                crate::hourly::Event::Helper(_) => {},
                                                _ => return Err(format!("unexpected fresh hourly event {event:?}")),
                                            }
                                            Ok(false)
                                        }).unwrap();
                                    false
                                });
                            }
                            crate::scheduler::Event::Helper(0x4d5b40) => {
                                context.update_clock(clock);
                                let previous = hooks.prompts.len();
                                                        let mut state = crate::level_script::State {
                            clock: clock,
                    ai_demand: &mut crate::ai_demand::State::default(),
                                                            objective: &mut objective, settings: &mut settings,
                                                            configuration: &mut config, welfare: &mut welfare,
                                                            pause: &mut pause, buildings: &mut buildings,
                                                            businesses: &mut ledgers.businesses, categories: &mut categories,
                                                            definitions: &mut definitions, commodities: &mut commodities,
                                                            catalog: &mut catalog, residents: &mut ledgers.residents,
                                                            creation: &mut creation, available: &mut available,
                                                            availability_cursor: &mut cursor, players: &[None; 6],
                    player_accounts: &mut [],
                                                            player_money: &mut cash, construction_components: &mut construction_components,
                                                        };
                                hourly_session.tick_events(&mut state, &context, &mut geometry, &mut hooks)?;
                                if hooks.prompts.len() != previous {
                                    prompt_ticks.push(clock.timing[0]);
                                }
                            }
                            crate::scheduler::Event::Helper(0x44c210) => {
                                road_traffic.advance(
                                    &mut road_network, &road_routes,
                                    crate::traffic::road::runtime::Tick {
                                        hour: clock.hour() as i32,
                                        era: clock.calendar[2],
                                        frequencies: &settings.traffic,
                                        camera: [0.; 3],
                                    }, &mut vehicle_models,
                                )?;
                            }
                            crate::scheduler::Event::Helper(address) => {
                                helper_boundaries.insert(address);
                            }
                            _ => return Err(format!("unexpected scheduler event {event:?}")),
                        }
                        Ok(0)
                        })();
                        std::mem::swap(tasks, &mut hooks.timers);
                        result
                    },
                )
                .unwrap();
                // Explicit diagnostic render fraction, after all logic work
                // for this host frame, including when the timer is paused.
                road_traffic.project(&mut road_network, 0.).unwrap();
                if pause.has_reason() && simulation_clock.timing[0] == tick {
                    paused_frames += 1;
                    assert!([300, 1250].contains(&tick));
                    assert_eq!(timed_tasks[0][0], 0);
                    if paused_frames % 3 == 0 {
                        std::mem::swap(&mut timed_tasks, &mut hooks.timers);
                        context.update_clock(&simulation_clock);
                        let mut state = crate::level_script::State {
                            clock: &mut simulation_clock,
                            ai_demand: &mut crate::ai_demand::State::default(),
                            objective: &mut objective,
                            settings: &mut settings,
                            configuration: &mut config,
                            welfare: &mut welfare,
                            pause: &mut pause,
                            buildings: &mut buildings,
                            businesses: &mut ledgers.businesses,
                            categories: &mut categories,
                            definitions: &mut definitions,
                            commodities: &mut commodities,
                            catalog: &mut catalog,
                            residents: &mut ledgers.residents,
                            creation: &mut creation,
                            available: &mut available,
                            availability_cursor: &mut cursor,
                            players: &[None; 6],
                            player_accounts: &mut [],
                            player_money: &mut cash,
                            construction_components: &mut construction_components,
                        };
                        hourly_session
                            .dispatch_catch_event(
                                ("InterfacePromptDismiss", 0),
                                &mut state,
                                &context,
                                &mut geometry,
                                &mut hooks,
                            )
                            .unwrap();
                        std::mem::swap(&mut timed_tasks, &mut hooks.timers);
                        assert!(!pause.has_reason());
                        assert_eq!(timed_tasks[0][0], 1);
                    }
                }
                saw_animation |= animations.records().iter().any(|row| row[4] != 0);
                for (index, node) in ledgers.residents.nodes[..count as usize].iter().enumerate() {
                    let refreshed = intent_refreshes[refresh_start..]
                        .iter()
                        .any(|&(_, resident, _)| resident == index);
                    let expected = if simulation_clock.timing[0] == tick {
                        countdowns_before[index]
                    } else if refreshed {
                        625
                    } else {
                        countdowns_before[index].wrapping_sub(1)
                    };
                    assert_eq!(
                        word(node.as_bytes(), 0xc0),
                        expected,
                        "tick {tick} resident {index}"
                    );
                }
            }
            assert_eq!(dispatches, [1251, 1251]);
            assert_eq!(paused_frames, if difficulty == 0 { 6 } else { 0 });
            assert_eq!(
                prompt_ticks.as_slice(),
                if difficulty == 0 {
                    &[300, 1250][..]
                } else {
                    &[]
                }
            );
            assert_eq!(
                hourly_session.events.pending().len(),
                if difficulty == 0 { 7 } else { 0 }
            );
            if difficulty == 0 {
                assert_eq!(
                    hooks.prompts,
                    [
                        vec![
                            Value::Number(1898.),
                            Value::Number(1899.),
                            Value::Number(1.),
                            Value::Number(0.),
                        ],
                        vec![
                            Value::Number(1900.),
                            Value::Number(1901.),
                            Value::Number(1.),
                            Value::Number(0.),
                        ],
                    ]
                );
                assert_eq!(hourly_session.events.pending()[0].remaining, 2600 - 1251);
            }
            assert_eq!(
                word(
                    &manager.players[0]
                        .auxiliaries
                        .iter()
                        .find(|a| a.reference == account_reference)
                        .unwrap()
                        .bytes,
                    0
                ),
                account_before - 2
            );
            assert_eq!(
                (road_traffic.signals.counter, road_traffic.signals.phase),
                (1251, 4)
            );
            assert_eq!(simulation_clock.hour(), 11);
            assert_eq!(
                intent_refreshes
                    .iter()
                    .filter(|&&(tick, _, hour)| tick == 1 && hour == 9)
                    .count(),
                count as usize
            );
            assert!(
                intent_refreshes
                    .iter()
                    .any(|&(tick, _, hour)| tick == 626 && hour == 10)
            );
            assert!(
                ledgers.residents.nodes[..count as usize]
                    .iter()
                    .any(|node| word(node.as_bytes(), 0xc0) == 0 && node.as_bytes()[0x7e] & 2 != 0)
            );
            assert_eq!(hourly_scripts, ["HourlyUpdate", "HourlyUpdate"]);
            let mut expected_hourly = Vec::new();
            for hour in [10, 11] {
                expected_hourly.extend([
                    crate::hourly::Event::Query,
                    crate::hourly::Event::Script {
                        name: 0x5414fc,
                        argument: hour,
                    },
                    crate::hourly::Event::Helper(0x4895b0),
                    crate::hourly::Event::Helper(0x44f2a0),
                    crate::hourly::Event::Categories,
                ]);
                if hour == 10 {
                    expected_hourly.push(crate::hourly::Event::Script {
                        name: 0x5414e4,
                        argument: 0,
                    });
                }
                expected_hourly.extend([
                    crate::hourly::Event::CurrentPlayerUpdate,
                    crate::hourly::Event::Helper(0x470b80),
                    crate::hourly::Event::Helper(0x4bb500),
                ]);
            }
            assert_eq!(hourly_events, expected_hourly);
            assert_eq!(&simulation_clock.timing[..3], &[1251, 1251, 1]);
            assert!(helper_boundaries.contains(&0x44ec50));
            assert!(saw_animation);
            assert!(
                creation.auxiliary.slots()[..count as usize]
                    .iter()
                    .zip(positions_before)
                    .any(|(row, before)| row[4..16] != before)
            );
            // Independent terminal consumer control. All timetables distinguish
            // stale hour11 (sleep) from the same-chunk write to hour13 (snack).
            let timetable = (0..100)
                .flat_map(|row| {
                    (1..=24).map(move |hour| {
                        format!(
                            "TIMETABLE {row} HOUR{hour} {}\n",
                            if hour == 13 { "SNACK_TIME" } else { "SLEEP" }
                        )
                    })
                })
                .collect::<String>();
            config.timetables = crate::resident::Timetables::from_timetable(&timetable).unwrap();
            simulation_clock.calendar[0] = 11;
            let before_count = ledgers.residents.active_count;
            let created_slot = ledgers
                .residents
                .nodes
                .iter()
                .position(|node| !node.is_active())
                .expect("free original resident slot");
            {
                let mut state = crate::level_script::State {
                    clock: &mut simulation_clock,
                    ai_demand: &mut crate::ai_demand::State::default(),
                    objective: &mut objective,
                    settings: &mut settings,
                    configuration: &mut config,
                    welfare: &mut welfare,
                    pause: &mut pause,
                    buildings: &mut buildings,
                    businesses: &mut ledgers.businesses,
                    categories: &mut categories,
                    definitions: &mut definitions,
                    commodities: &mut commodities,
                    catalog: &mut catalog,
                    residents: &mut ledgers.residents,
                    creation: &mut creation,
                    available: &mut available,
                    availability_cursor: &mut cursor,
                    players: &[None; 6],
                    player_accounts: &mut ledgers.players,
                    player_money: &mut cash,
                    construction_components: &mut construction_components,
                };
                hourly_session
                    .runtime
                    .execute(
                        "live resident hour",
                        b"Calendar.hour=13; People.Create(1)",
                        &mut |id, args| {
                            hourly_session.bindings.handle(
                                id,
                                args,
                                &mut state,
                                &mut hourly_session.events,
                                &context,
                                &mut geometry,
                                &mut hooks,
                            )
                        },
                    )
                    .unwrap();
                assert_eq!(state.residents.active_count, before_count + 1);
                let resident = &state.residents.nodes[created_slot];
                assert_eq!(
                    word(resident.as_bytes(), 0xcc),
                    8,
                    "actual initialized snack state at hour13"
                );
            }
        }
    }
}
