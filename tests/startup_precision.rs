//! Synthetic PC53/PC64 witnesses; no supplied game files are needed.
use openmt2001::{
    building_binding, building_parameters::BuildingKinds, pe::PeImage, x87::Precision,
};
#[path = "support/building_image.rs"]
mod building_image;
#[cfg(feature = "lua4")]
use openmt2001::construction;

fn quote_image() -> Vec<u8> {
    let mut bytes = building_image::fixture();
    let at = 0x200 + (0x5365f0 - 0x525000) + 2 * 46;
    bytes[at..at + 4].fill(0);
    bytes
}
fn utility_business() -> [u8; 0x74] {
    let mut business = [0; 0x74];
    for (offset, value) in [(0x60, 2f32.powi(-54)), (0x64, 1. + 2f32.powi(-12))] {
        business[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    business
}
#[cfg(feature = "lua4")]
fn command() -> construction::Command {
    construction::Command {
        category: 0,
        rectangle: [0; 4],
        height: 1,
        kind: 2,
        business: 1,
        variation: 0,
        quality: 0,
        owner: 1,
        components: [0.; 4],
    }
}

#[test]
fn startup_precision_quotes_keep_component_bits_and_command_snapshot() {
    let bytes = quote_image();
    let image = PeImage::parse(&bytes).unwrap();
    let parameters = BuildingKinds::from_image(&image).unwrap();
    let business = utility_business();
    for (precision, expected) in [(Precision::Pc53, 0x45801000), (Precision::Pc64, 0x45801001)] {
        let input = building_binding::ConstructionInputs {
            precision,
            volume: 4097,
            kind: 2,
            quality: 0,
            category_value: 0.,
            hour: 6,
            discount: 0.,
            owner: 1,
            utility_owners: [0; 4],
            business: &business,
            products: &[],
        };
        let configured =
            building_binding::construction_cost_with_parameters(&input, &parameters, &image)
                .unwrap();
        assert_eq!(configured.components[3].to_bits(), expected);
        assert_eq!(configured.total, 4098);
        assert_eq!(
            building_binding::construction_cost(&input, &image)
                .unwrap()
                .components
                .map(f32::to_bits),
            configured.components.map(f32::to_bits)
        );
        #[cfg(feature = "lua4")]
        {
            let mut command = command();
            let quoted = openmt2001::live_commands::quote_components(
                &mut command,
                &input,
                &parameters,
                &image,
            )
            .unwrap();
            assert_eq!(quoted.total, 4098);
            let packet = command.packet();
            assert_eq!(
                construction::Command::from_packet(&packet)
                    .unwrap()
                    .components[3]
                    .to_bits(),
                expected
            );
        }
    }
}

#[cfg(feature = "lua4")]
#[test]
fn startup_precision_ai_initialization_and_in_play_events_choose_their_context() {
    use openmt2001::{
        ai_script::{AiScript, Initialization},
        player_manager::{Manager, Player},
    };
    let cash = f32::from_bits(0x5b000001);
    let source = br#"
function EVENT_LoadHseKeepingTimetable(a) end
function EVENT_LoadBuyBlockTimetable(a) end
function EVENT_LoadDifficultyVariables(a) AiChars[a]:IncreaseMoney(2147483647) end
function EVENT_LoadBuildTimes(a) end
function EVENT_LoadScoreData() end
function EVENT_Money(a) AiChars[a]:IncreaseMoney(2147483647) end
"#;
    for (precision, expected) in [(Precision::Pc53, 0x5b000002), (Precision::Pc64, 0x5b000001)] {
        let mut scripts = AiScript::new().unwrap();
        let mut manager = Manager::initialize([0; 40]);
        let mut bytes = vec![0; 0x1e50];
        bytes[4..8].copy_from_slice(&cash.to_le_bytes());
        manager.players.push(Player {
            reference: 0x10000000,
            bytes,
            auxiliaries: vec![],
        });
        let mut profiles = [[0; 0x650]; 10];
        profiles[0][..4].copy_from_slice(&0x10000000u32.to_le_bytes());
        profiles[0][4] = 1;
        scripts
            .initialize(
                &mut Initialization::<64> {
                    precision,
                    profiles: &mut profiles,
                    manager: &mut manager,
                    category_counts: &mut [0; 52 * 45],
                    chains: &std::array::from_fn(|_| vec![]),
                    buildings: &[],
                    businesses: &[],
                    difficulty: 0,
                    current_hour: 0,
                    count_tick: None,
                },
                source,
                None,
            )
            .unwrap();
        assert_eq!(
            u32::from_le_bytes(manager.players[0].bytes[4..8].try_into().unwrap()),
            expected
        );
        manager.players[0].bytes[4..8].copy_from_slice(&cash.to_le_bytes());
        scripts
            .trigger_timetable(&mut profiles, &mut manager.players, "Money", 0, 6)
            .unwrap();
        assert_eq!(
            u32::from_le_bytes(manager.players[0].bytes[4..8].try_into().unwrap()),
            0x5b000001
        );
    }
}

#[test]
fn startup_precision_money_preserves_signed_add_and_set_semantics() {
    use openmt2001::economy::{ScriptMoneyOperation::*, script_money};
    for operation in [Increase, Decrease] {
        for (precision, expected) in [(Precision::Pc53, 0x5b000002), (Precision::Pc64, 0x5b000001)]
        {
            assert_eq!(
                script_money(
                    f32::from_bits(0x5b000001),
                    f64::from(i32::MAX),
                    operation,
                    precision
                )
                .to_bits(),
                expected
            );
            assert_eq!(script_money(100., -10.75, operation, precision), 90.);
            assert_eq!(script_money(100., 4294967295., operation, precision), 99.);
            assert_eq!(
                script_money(100., f64::INFINITY, operation, precision),
                100.
            );
        }
    }
    for precision in [Precision::Pc53, Precision::Pc64] {
        assert_eq!(script_money(100., 4294967295., Set, precision), -1.);
        assert_eq!(script_money(100., f64::NAN, Set, precision), 0.);
    }
}

#[test]
fn startup_precision_network_sampler_uses_incoming_precision_only_for_fiadd() {
    use openmt2001::{economy::PlayerAccount, network::synchronization::sample};
    let account = |cash: f32| {
        let mut bytes = [0; 0x1bc];
        bytes[4..8].copy_from_slice(&cash.to_le_bytes());
        PlayerAccount::from_runtime(&bytes).unwrap()
    };
    // Use counters to construct an exact i32 MAX sum.
    let mut first = account(0.);
    first.retail_counters[0] = i32::MAX;
    let second = account(2f32.powi(55));
    let rows = [(true, &first), (true, &second)];
    assert_eq!(
        sample::players_with_precision(rows, Precision::Pc53),
        0x80000000
    );
    assert_eq!(
        sample::players_with_precision(rows, Precision::Pc64),
        0x7fffffff
    );
    assert_eq!(sample::players(rows), 0x7fffffff);
    let mut row = [0; 0x394];
    row[0x3f] = 1;
    row[0xc..0x10].copy_from_slice(&(-1.75f32).to_le_bytes());
    row[0x10..0x14].copy_from_slice(&2.75f32.to_le_bytes());
    assert_eq!(sample::buildings(&[row]), 1); // FLD/FISTP has no arithmetic rounding.
}

#[cfg(feature = "lua4")]
#[test]
fn startup_precision_map_add_building_forwards_the_invocation_context() {
    use openmt2001::{building_pool, business_binding, business_pool, map_script};
    struct Hooks;
    impl business_pool::BusinessPoolHooks for Hooks {
        fn owner_transfer(&mut self, _: usize, _: &mut [u8; 544], _: u32) -> Result<(), String> {
            panic!("unexpected transfer")
        }
        fn notify(&mut self) -> Result<(), String> {
            panic!("unexpected notification")
        }
    }
    impl map_script::Hooks for Hooks {
        fn building_appearance(&mut self, _: usize, _: &mut [u8; 916]) -> Result<(), String> {
            Ok(())
        }
        fn building_created(&mut self, _: usize) -> Result<(), String> {
            Ok(())
        }
        fn binding_call(
            &mut self,
            _: business_binding::Call,
            _: &mut business_binding::World<'_>,
        ) -> Result<(), String> {
            panic!("unexpected business binding")
        }
        fn stock_charge(&mut self, _: &mut [u8; 544], _: f32) -> Result<(), String> {
            panic!("unexpected stock charge")
        }
    }
    struct Geometry;
    impl building_pool::GeometryBinding for Geometry {
        fn prepare(&mut self, _: usize, _: &mut [u8; 916]) -> Result<(), String> {
            Ok(())
        }
        fn model_height(&mut self, _: u32) -> Result<f32, String> {
            Ok(10.)
        }
    }
    let bytes = quote_image();
    let image = PeImage::parse(&bytes).unwrap();
    let parameters = BuildingKinds::from_image(&image).unwrap();
    let mut rows = [[0; 116]; 45];
    rows[1] = utility_business();
    let catalog = business_pool::BusinessCatalog::from_rows(rows);
    for (precision, expected) in [(Precision::Pc53, 0x45801000), (Precision::Pc64, 0x45801001)] {
        let mut buildings = vec![[0; 916]; 600];
        building_pool::reset(&mut buildings, &mut [0; 916]).unwrap();
        let mut categories = [[0; 380]; 52];
        let mut businesses = vec![[0; 544]; 600];
        let mut available = vec![];
        let mut cursor = None;
        let mut state = map_script::State {
            precision,
            building_parameters: parameters,
            world: business_binding::World {
                buildings: &mut buildings,
                businesses: &mut businesses,
                categories: &mut categories,
                category_base: 0x617628,
                catalog: &catalog,
                available: &mut available,
                availability_cursor: &mut cursor,
                players: &[],
                resident_count: 0,
            },
            runtime: business_binding::Runtime {
                building_base: 0x591318,
                products: &[[[0; 2]; 6]; 45],
                commodities: &[[0; 28]; 32],
                residents: &mut [],
                timestamp: 0,
            },
            image: &image,
            hour: 6,
            discount: 0.,
            date: [0; 2],
            ownership_timestamp: 0,
            // Distinct from AddBuilding's owner zero, as in the native witness.
            utility_owners: [1; 4],
            notifications: false,
            construction_components: [0.; 4],
        };
        assert_eq!(
            state
                .add_building(
                    &map_script::Building {
                        category: 0,
                        rectangle: [0; 4],
                        height: 4097,
                        volume: 4097,
                        kind: 2,
                        variation: 0,
                        quality: 0,
                    },
                    &mut Geometry,
                    &mut Hooks
                )
                .unwrap(),
            Some(0)
        );
        assert_eq!(state.construction_components[3].to_bits(), expected);
        assert_eq!(
            u32::from_le_bytes(state.world.buildings[0][0x38..0x3c].try_into().unwrap()),
            4098f32.to_bits()
        );
    }
}
