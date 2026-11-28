//! Literal expectations recovered from v1.4 445720,4450e0,463560,445520.
//! This is an authored inert PE fixture, never original executable code.
use openmt2001::{building_binding, building_pool, demolition, pe::PeImage};

#[path = "support/building_image.rs"]
mod building_image;
use building_image::fixture;

fn initialization(kind: u32) -> building_pool::Initialization {
    building_pool::Initialization {
        rectangle: [0, 0, 1, 0],
        height: 1,
        variation: 0,
        kind,
        quality: 0,
        center: [0.0; 2],
        total_cost: 20f32.to_bits(),
        date: [0; 2],
    }
}

#[test]
fn literal_image_primitives_keep_original_unconfigured_rates() {
    let bytes = fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let business = [0; 0x74];
    for kind in 0..3 {
        for (hour, expected, rent) in [(5, 20, 0.0f32), (6, 40, 20.0)] {
            let cost = building_binding::construction_cost(
                &building_binding::ConstructionInputs {
                    volume: 2,
                    kind,
                    quality: 0,
                    category_value: 0.0,
                    hour,
                    discount: 0.0,
                    owner: 1,
                    utility_owners: [1; 4],
                    business: &business,
                    products: &[],
                },
                &image,
            )
            .unwrap();
            assert_eq!(cost.total, expected);
            assert_eq!(
                cost.components.map(f32::to_bits),
                [20.0f32, 0.0, rent, 0.0].map(f32::to_bits)
            );
        }
        let mut building = [0; 916];
        building[..4].copy_from_slice(&17u32.to_le_bytes());
        building[0x90] = 0xa5;
        building_pool::initialize(&mut building, &initialization(kind), &image).unwrap();
        assert_eq!(&building[..4], &17u32.to_le_bytes());
        assert_eq!(building[0x90], 0xa5);
        assert_eq!(&building[0x28..0x2c], &10u32.to_le_bytes());
        assert_eq!(
            building_binding::initial_rent(&building, 0.0, 0.0, &image).unwrap(),
            20
        );
    }
}

#[test]
fn initial_rent_consumes_materialized_saved_word() {
    let bytes = fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let mut building = [0; 916];
    building_pool::initialize(&mut building, &initialization(0), &image).unwrap();
    // Model an imported saved prefix; no initializer runs after restoration.
    for (stored_rate, expected) in [(10u32, 20u16), (3, 6), (0, 0)] {
        building[0x28..0x2c].copy_from_slice(&stored_rate.to_le_bytes());
        assert_eq!(
            building_binding::initial_rent(&building, 0.0, 0.0, &image).unwrap(),
            expected
        );
        assert_eq!(&building[0x28..0x2c], &stored_rate.to_le_bytes());
    }
}

#[test]
fn raw_demolition_uses_zero_default_and_inclusive_volume() {
    let bytes = fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let mut building = [0; 916];
    building[4..8].copy_from_slice(&[0, 0, 1, 2]);
    building[8] = 2;
    building[0x40..0x44].copy_from_slice(&0x617628u32.to_le_bytes());
    let mut categories = [[0; 380]; 52];
    categories[0][0x84..0x88].copy_from_slice(&2f32.to_le_bytes());
    assert_eq!(
        demolition::quote(&building, &categories, &image)
            .unwrap()
            .to_f32()
            .to_bits(),
        0f32.to_bits()
    );
}

const CONFIGURED: &str = "BuildingRent Residential 3
BuildingCostPerUnit Residential 15
BuildingCostToDemolish Residential 4
BuildingRent Retail 3
BuildingCostPerUnit Retail 15
BuildingCostToDemolish Retail 4
BuildingRent Landscape 0
BuildingCostPerUnit Landscape 15
BuildingCostToDemolish Landscape 4
";

#[test]
fn configured_rows_and_binary32_first_low_word_writes() {
    use openmt2001::building_parameters::BuildingKinds;
    let bytes = fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let raw = BuildingKinds::from_image(&image).unwrap();
    assert_eq!(
        [
            raw.row(0).unwrap(),
            raw.row(1).unwrap(),
            raw.row(2).unwrap()
        ],
        [[10, 10, 0]; 3]
    );
    let parameters = BuildingKinds::from_parameters(CONFIGURED, &image).unwrap();
    assert_eq!(
        [
            parameters.row(0).unwrap(),
            parameters.row(1).unwrap(),
            parameters.row(2).unwrap()
        ],
        [[3, 15, 4], [3, 15, 4], [0, 15, 4]]
    );
    let source = "buildingrent ReTaIl 9
BuildingRent Retail 3.9
BuildingCostPerUnit Residential -1.9
BuildingCostToDemolish Landscape 16777217
BuildingRent Retail
BuildingCostPerUnit Retail 65537
";
    let changed = BuildingKinds::from_parameters(source, &image).unwrap();
    assert_eq!(changed.row(0).unwrap(), [3, 1, 0]);
    assert_eq!(changed.row(1).unwrap(), [10, 65535, 0]);
    assert_eq!(changed.row(2).unwrap(), [10, 10, 0]);
    assert!(changed.row(3).is_err());
    for source in [
        "BuildingRent Missing 1",
        "BuildingRent Retail junk",
        "BuildingCostPerUnit Retail 3garbage",
        "BuildingRent Retail NaN",
        "BuildingRent Retail inf",
        "BuildingRent Retail 1e100",
        "BuildingRent Rétaïl 3",
    ] {
        assert!(
            BuildingKinds::from_parameters(source, &image).is_err(),
            "{source}"
        );
    }
    // Native signed64 integer-indefinite has zero low word for huge finite f32.
    assert_eq!(
        BuildingKinds::from_parameters("BuildingRent Retail 1e30", &image)
            .unwrap()
            .row(0)
            .unwrap()[0],
        0
    );
}

#[test]
fn property_lookup_uses_last_case_insensitive_match() {
    use openmt2001::building_parameters::BuildingKinds;
    let mut bytes = fixture();
    let offset = 0x200 + (0x5365d0 - 0x525000) + 2 * 46;
    bytes[offset..offset + 32].fill(0);
    bytes[offset..offset + 6].copy_from_slice(b"retail");
    let image = PeImage::parse(&bytes).unwrap();
    let parameters = BuildingKinds::from_parameters("BuildingRent RETAIL 7", &image).unwrap();
    assert_eq!(parameters.row(0).unwrap(), [10, 10, 0]);
    assert_eq!(parameters.row(2).unwrap(), [7, 10, 0]);
}

#[test]
fn configured_quote_initializer_and_demolition_have_literal_amounts() {
    use openmt2001::building_parameters::BuildingKinds;
    let bytes = fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let parameters = BuildingKinds::from_parameters(CONFIGURED, &image).unwrap();
    let business = [0; 0x74];
    for kind in 0..3 {
        for (hour, total, rent) in [
            (5, 30, 0.0f32),
            (
                6,
                if kind == 2 { 30 } else { 36 },
                if kind == 2 { 0.0 } else { 6.0 },
            ),
        ] {
            let cost = building_binding::construction_cost_with_parameters(
                &building_binding::ConstructionInputs {
                    volume: 2,
                    kind,
                    quality: 0,
                    category_value: 0.0,
                    hour,
                    discount: 0.0,
                    owner: 1,
                    utility_owners: [1; 4],
                    business: &business,
                    products: &[],
                },
                &parameters,
                &image,
            )
            .unwrap();
            assert_eq!(cost.total, total);
            assert_eq!(
                cost.components.map(f32::to_bits),
                [30.0f32, 0.0, rent, 0.0].map(f32::to_bits)
            );
        }
        let mut new = [0; 916];
        new[..4].copy_from_slice(&17u32.to_le_bytes());
        new[0x90] = 0xa5;
        building_pool::initialize_with_parameters(&mut new, &initialization(kind), &parameters)
            .unwrap();
        let rate = if kind == 2 { 0u32 } else { 3 };
        assert_eq!(&new[0x28..0x2c], &rate.to_le_bytes());
        assert_eq!(&new[..4], &17u32.to_le_bytes());
        assert_eq!(new[0x90], 0xa5);
        assert_eq!(
            building_binding::initial_rent(&new, 0.0, 0.0, &image).unwrap(),
            if kind == 2 { 0 } else { 6 }
        );
        new[4..8].copy_from_slice(&[0, 0, 1, 2]);
        new[8] = 2;
        new[0x40..0x44].copy_from_slice(&0x617628u32.to_le_bytes());
        let mut categories = [[0; 380]; 52];
        categories[0][0x84..0x88].copy_from_slice(&2f32.to_le_bytes());
        assert_eq!(
            demolition::quote_with_parameters(&new, &categories, &parameters, &image)
                .unwrap()
                .to_f32()
                .to_bits(),
            96f32.to_bits()
        );
    }
}

#[test]
#[ignore = "requires inert original PE and parameter.prm"]
fn supplied_original_parameter_rows() {
    use openmt2001::building_parameters::BuildingKinds;
    let bytes = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
    let source = std::fs::read_to_string(openmt2001::fixtures::path("PARAMETERS")).unwrap();
    let image = PeImage::parse(&bytes).unwrap();
    let raw = BuildingKinds::from_image(&image).unwrap();
    assert_eq!(
        [
            raw.row(0).unwrap(),
            raw.row(1).unwrap(),
            raw.row(2).unwrap()
        ],
        [[10, 10, 0]; 3]
    );
    let configured = BuildingKinds::from_parameters(&source, &image).unwrap();
    assert_eq!(
        [
            configured.row(0).unwrap(),
            configured.row(1).unwrap(),
            configured.row(2).unwrap()
        ],
        [[3, 15, 4], [3, 15, 4], [0, 15, 4]]
    );
}

struct Geometry;
impl building_pool::GeometryBinding for Geometry {
    fn prepare(&mut self, _: usize, _: &mut [u8; 916]) -> Result<(), String> {
        Ok(())
    }
    fn model_height(&mut self, _: u32) -> Result<f32, String> {
        Ok(10.0)
    }
}

#[test]
fn due_build_keeps_descriptor_cost_and_copies_current_resource_rent() {
    use openmt2001::{
        building_parameters::BuildingKinds, construction, economy::PlayerAccount, pending_world,
    };
    let bytes = fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let parameters = BuildingKinds::from_parameters(
        "BuildingRent Retail 3\nBuildingCostPerUnit Retail 999",
        &image,
    )
    .unwrap();
    let command = construction::Command {
        category: 0,
        rectangle: [0, 0, 1, 0],
        height: 1,
        kind: 0,
        business: 0,
        variation: 0,
        quality: 0,
        owner: 1,
        components: [100.0, 7.0, 6.0, 10.0],
    };
    // Transport roundtrip models already admitted queue ownership; the due
    // consumer must not replace123 with a fresh quote at changed cost999.
    let command = construction::Command::from_packet(&command.packet()).unwrap();
    let mut buildings = vec![[0; 916]; 600];
    building_pool::reset(&mut buildings, &mut [0; 916]).unwrap();
    let mut categories = [[0; 380]; 52];
    let mut pending = pending_world::State::default();
    let mut raw_player = [0; 0x1bc];
    raw_player[4..8].copy_from_slice(&1000f32.to_le_bytes());
    let mut players = [PlayerAccount::from_runtime(&raw_player).unwrap()];
    let mut posts = Vec::new();
    let slot = construction::apply(
        &command,
        &mut construction::World {
            buildings: &mut buildings,
            categories: &mut categories,
            pending: &mut pending,
            players: &mut players,
        },
        &construction::Context {
            building_parameters: parameters,
            image: &image,
            building_base: 0x591318,
            category_base: 0x617628,
            date: [0; 2],
            ownership_timestamp: 9,
        },
        &mut Geometry,
        |event, _| {
            if let construction::Event::Posted { amount, ledger, .. } = event {
                posts.push((amount.to_bits(), ledger));
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(slot, 0);
    assert_eq!(&buildings[slot][0x28..0x2c], &3u32.to_le_bytes());
    assert_eq!(&buildings[slot][0x38..0x3c], &123f32.to_le_bytes());
    assert_eq!(players[0].cash.to_bits(), 877f32.to_bits());
    assert_eq!(
        posts,
        vec![
            ((-100f32).to_bits(), 0xc),
            ((-6f32).to_bits(), 0xe),
            ((-7f32).to_bits(), 0x11),
            ((-10f32).to_bits(), 7)
        ]
    );
    assert_eq!(pending.count, 1);
}

#[cfg(feature = "lua4")]
#[test]
fn map_producer_quotes_and_materializes_configured_rows() {
    use openmt2001::{
        building_parameters::BuildingKinds, business_binding, business_pool, map_script,
    };
    struct Hooks(Vec<usize>);
    impl business_pool::BusinessPoolHooks for Hooks {
        fn owner_transfer(&mut self, _: usize, _: &mut [u8; 544], _: u32) -> Result<(), String> {
            panic!("unexpected business transfer")
        }
        fn notify(&mut self) -> Result<(), String> {
            panic!("unexpected business notify")
        }
    }
    impl map_script::Hooks for Hooks {
        fn building_appearance(&mut self, _: usize, _: &mut [u8; 916]) -> Result<(), String> {
            panic!("unexpected appearance without model")
        }
        fn building_created(&mut self, slot: usize) -> Result<(), String> {
            self.0.push(slot);
            Ok(())
        }
        fn binding_call(
            &mut self,
            _: business_binding::Call,
            _: &mut business_binding::World<'_>,
        ) -> Result<(), String> {
            panic!("unexpected business bind")
        }
        fn stock_charge(&mut self, _: &mut [u8; 544], _: f32) -> Result<(), String> {
            panic!("unexpected stock charge")
        }
    }
    let bytes = fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let parameters = BuildingKinds::from_parameters(CONFIGURED, &image).unwrap();
    let catalog = business_pool::BusinessCatalog::from_rows([[0; 116]; 45]);
    let mut buildings = vec![[0; 916]; 600];
    building_pool::reset(&mut buildings, &mut [0; 916]).unwrap();
    let mut categories = [[0; 380]; 52];
    let mut businesses = vec![[0; 544]; 600];
    let mut available = Vec::new();
    let mut cursor = None;
    let mut state = map_script::State {
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
        discount: 0.0,
        date: [0; 2],
        ownership_timestamp: 0,
        utility_owners: [0; 4],
        notifications: false,
        construction_components: [0.0; 4],
    };
    let mut hooks = Hooks(Vec::new());
    assert_eq!(
        state
            .add_building(
                &map_script::Building {
                    category: 0,
                    rectangle: [0, 0, 1, 0],
                    height: 1,
                    volume: 2,
                    kind: 0,
                    variation: 0,
                    quality: 0,
                },
                &mut Geometry,
                &mut hooks
            )
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        state.construction_components.map(f32::to_bits),
        [30.0f32, 0.0, 6.0, 0.0].map(f32::to_bits)
    );
    assert_eq!(&state.world.buildings[0][0x28..0x2c], &3u32.to_le_bytes());
    assert_eq!(&state.world.buildings[0][0x38..0x3c], &36f32.to_le_bytes());
    assert_eq!(hooks.0, vec![0]);
}
