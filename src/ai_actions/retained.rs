//! Bind47c630 to authoritative catalog, resident, geometry and proposal state.
use crate::{
    business_pool::BusinessCatalog,
    economy::ResidentAccounts,
    pe::PeImage,
    player_manager::{Player, word},
};
pub struct World<'a> {
    pub building_parameters: crate::building_parameters::BuildingKinds,
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub catalog: &'a BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub residents: &'a ResidentAccounts,
    ///Manager-order owner references, including the currently borrowed player.
    pub player_references: &'a [u32],
    pub player_count: i32,
    pub category_counts: &'a mut [u32; 52 * 45],
    pub proposal_identity: &'a mut u32,
    pub construction_components: &'a mut [f32; 4],
    pub hour: i32,
    pub mode: u32,
    pub allowance: i32,
    pub affordability_override: bool,
    pub discount: f32,
    ///Host policy for bytes the original proposal stack constructor never writes.
    pub proposal_prior: [u8; 104],
}
pub(crate) fn quote(
    arguments: &[u32; 10],
    player: &Player,
    world: &World<'_>,
    image: &PeImage<'_>,
) -> Result<crate::building_binding::ConstructionCost, String> {
    let category = world
        .categories
        .get(arguments[0] as usize)
        .ok_or("AI construction quote category missing")?;
    let kind = arguments[2];
    let business = arguments[3] as usize;
    let info = world
        .catalog
        .row(business)
        .ok_or("AI construction quote business missing")?;
    let mut owners = [0; 4];
    for (out, id) in owners.iter_mut().zip([2, 35, 39, 42]) {
        let owner = world.categories[id][5];
        if owner != 0 {
            *out = *world
                .player_references
                .get(usize::from(owner) - 1)
                .ok_or("AI construction utility owner missing")?;
        }
    }
    let mut products = vec![];
    if world.hour >= 6 && kind == 0 {
        for entry in world
            .products
            .get(business)
            .and_then(|p| p.get(..usize::from(info[0x4c])))
            .ok_or("AI construction products exceed catalog")?
        {
            let row = world
                .commodities
                .get(entry[0] as usize)
                .ok_or("AI construction commodity missing")?;
            products.push(crate::building_binding::StockProduct {
                value: f32::from_bits(word(row, 0x14)),
                fraction: f32::from_bits(entry[1]),
            });
        }
    }
    crate::building_binding::construction_cost_with_parameters(
        &crate::building_binding::ConstructionInputs {
            volume: arguments[1] as i32,
            kind,
            quality: arguments[4] as i8,
            category_value: f32::from_bits(word(category, 0x84)),
            hour: world.hour,
            discount: world.discount,
            owner: player.reference,
            utility_owners: owners,
            business: info,
            products: &products,
        },
        &world.building_parameters,
        image,
    )
}
///Only native debug diagnostics leave this adapter. All gameplay decisions,
///placements, affordability checks and proposal insertion run synchronously.
pub fn choose(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    input: super::Input,
    state: &mut super::Planning,
    world: &mut World<'_>,
    mut diagnostic: impl FnMut(super::Call, &mut Player, &mut [[u8; 0x650]]) -> Result<(), String>,
) -> Result<bool, String> {
    super::choose_from_catalog(
        player,
        profiles,
        world.categories,
        world.catalog,
        input,
        state,
        |call, player, profiles, categories, state| {
            let a = call.arguments;
            let mut reply = super::Reply::default();
            reply.value = match call.address {
                0x4769f0 => u32::from(crate::ai_hourly::delay_signal(world.mode)),
                0x47dd40 => super::planning::preferred_area_from_catalog(
                    a[0],
                    a[1],
                    profiles,
                    world.catalog,
                )?,
                0x47db30 => super::planning::floor_count_from_catalog(
                    player,
                    a[0],
                    profiles,
                    world.catalog,
                )?,
                0x474080 | 0x474100 => {
                    super::planning::housing_limit(a[0], call.address == 0x474100, image)?
                }
                0x47dd90 => super::planning::style(
                    player,
                    categories
                        .get(a[1] as usize)
                        .ok_or("AI planning style category missing")?,
                    world.mode as i32,
                )?,
                0x476ee0 => {
                    super::demand::clamp(
                        player,
                        &mut state.area,
                        &mut state.floors,
                        profiles,
                        world.catalog,
                        world.products,
                        world.player_count,
                        image,
                        |id| super::demand::housed_demand(id, world.residents, world.commodities),
                    )?;
                    0
                }
                0x46ba80 => {
                    super::planning::footprints_from_catalog(
                        a[0],
                        &mut state.footprints,
                        &mut state.count,
                        world.catalog,
                    )?;
                    0
                }
                0x46bd50 => {
                    super::planning::trim_footprints(
                        a[0],
                        &mut state.footprints,
                        &mut state.count,
                    )?;
                    0
                }
                0x475f70 => {
                    super::planning::reduce_from_catalog(
                        player,
                        a[1],
                        &mut state.floors,
                        &mut state.area,
                        profiles,
                        world.catalog,
                    )?;
                    0
                }
                0x46ee90 => {
                    super::planning::normalize_area_from_catalog(
                        player,
                        a[0],
                        &mut state.area,
                        profiles,
                        world.catalog,
                    )?;
                    0
                }
                0x476b30 => u32::from(super::planning::reserve_blocked(
                    player,
                    a[0] as i32,
                    image,
                )?),
                0x4778c0 => super::landscape::remaining(
                    categories
                        .get(a[0] as usize)
                        .ok_or("AI landscape allowance category missing")?,
                    world.buildings,
                )? as u32,
                0x47de90 => u32::from(super::landscape::place_in_category(
                    player,
                    categories
                        .get(a[1] as usize)
                        .ok_or("AI landscape placement category missing")?,
                    world.buildings,
                    &mut state.position,
                    &mut state.width,
                    &mut state.height,
                )?),
                0x445ef0 => {
                    let category = categories
                        .get(word(&player.bytes, 0xb68) as usize)
                        .ok_or("AI landscape validation category missing")?;
                    let grid = category[0x9c..0xcd].try_into().unwrap();
                    u32::from(crate::building_pool::can_place(
                        grid,
                        [
                            a[0] as i32,
                            a[1] as i32,
                            a[0].wrapping_add(a[2]).wrapping_sub(1) as i32,
                            a[1].wrapping_add(a[3]).wrapping_sub(1) as i32,
                        ],
                    ))
                }
                0x4747c0 => {
                    let owner = a[4];
                    if world.player_references.get(owner as usize) != Some(&player.reference) {
                        return Err("AI placement owner differs from selected player".into());
                    }
                    u32::from(super::placement::place_in_category(
                        player,
                        categories
                            .get(a[0] as usize)
                            .ok_or("AI placement category missing")?,
                        &mut state.position,
                        &mut state.width,
                        &mut state.height,
                    )?)
                }
                0x445720 => {
                    let cost = quote(&a, player, world, image)?;
                    *world.construction_components = cost.components;
                    reply.quote = cost.total as f32;
                    0
                }
                0x48a6d0 => u32::from(crate::construction::affordable(
                    f32::from_bits(word(&player.bytes, 4)),
                    world.allowance,
                    f32::from_bits(a[0]),
                    world.affordability_override,
                )),
                0x478650 => u32::from(super::proposals::construction(
                    player,
                    a,
                    world.proposal_prior,
                    world.proposal_identity,
                    |call, player, _proposal| {
                        let super::proposals::Call::Diagnostic(address) = call else {
                            return Err("AI construction proposal unexpectedly requested non-debug callback".into());
                        };
                        diagnostic(
                            super::Call::new(0x4e6960, &[1, 0, address]),
                            player,
                            profiles,
                        )?;
                        Ok(crate::x87::Sum::from_i32(0))
                    },
                )?),
                0x4725f0 => {
                    let index = (a[0] as usize)
                        .checked_mul(45)
                        .and_then(|i| i.checked_add(a[1] as usize))
                        .ok_or("AI construction category count overflow")?;
                    let count = world
                        .category_counts
                        .get_mut(index)
                        .ok_or("AI construction category count leaves table")?;
                    *count = count.wrapping_add(1);
                    0
                }
                0x50e94f | 0x4e6960 => {
                    diagnostic(call, player, profiles)?;
                    0
                }
                other => {
                    return Err(format!(
                        "AI retained planning callback{other:08x} unresolved"
                    ));
                }
            };
            Ok(reply)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires full original planning execution observations"]
    fn matches_native_system() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PLANNING_SYSTEM").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAISYS1");
        assert_eq!(r.word(), 0x37f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let input = super::super::Input {
                execute: r.word() as u8,
                style_override: r.word() as u8,
                style: r.word(),
            };
            let hour = r.word() as i32;
            let mode = r.word();
            let expected = r.word();
            let identity_after = r.word();
            let n = r.word();
            let diagnostics: Vec<_> = (0..n).map(|_| r.word()).collect();
            let prior = r.blob().try_into().unwrap();
            let before: Vec<_> = (0..12).map(|_| r.blob()).collect();
            let after: Vec<_> = (0..12).map(|_| r.blob()).collect();
            let mut player = Player {
                reference: 0x10010000,
                bytes: before[0].to_vec(),
                auxiliaries: vec![],
            };
            let mut profiles: Vec<[u8; 0x650]> = before[1]
                .chunks_exact(0x650)
                .map(|x| x.try_into().unwrap())
                .collect();
            let categories = before[2]
                .chunks_exact(380)
                .map(|x| x.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let catalog = BusinessCatalog::from_rows(
                before[3]
                    .chunks_exact(116)
                    .map(|x| x.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let products = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    [
                        word(before[4], i * 48 + j * 8),
                        word(before[4], i * 48 + j * 8 + 4),
                    ]
                })
            });
            let residents = ResidentAccounts {
                active_count: 6,
                nodes: before[5]
                    .chunks_exact(336)
                    .map(|b| {
                        crate::resident::Resident::from_pool_bytes(
                            b[..0x14a].try_into().unwrap(),
                            b[0x14a],
                        )
                    })
                    .collect(),
                accounts: before[6]
                    .chunks_exact(144)
                    .map(|b| crate::resident::SpendingAccount::from_bytes(b.try_into().unwrap()))
                    .collect(),
            };
            let commodities = before[7]
                .chunks_exact(28)
                .map(|x| x.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let buildings: Vec<[u8; 916]> = before[8]
                .chunks_exact(916)
                .map(|x| x.try_into().unwrap())
                .collect();
            let mut counts = std::array::from_fn(|i| word(before[9], i * 4));
            let mut components = std::array::from_fn(|i| f32::from_bits(word(before[10], i * 4)));
            let mut identity = 0;
            let mut world = World {
                building_parameters: crate::building_parameters::BuildingKinds::from_image(&image)
                    .unwrap(),
                categories: &categories,
                buildings: &buildings,
                catalog: &catalog,
                products: &products,
                commodities: &commodities,
                residents: &residents,
                player_references: &[0x10010000],
                player_count: 4,
                category_counts: &mut counts,
                proposal_identity: &mut identity,
                construction_components: &mut components,
                hour,
                mode,
                allowance: 10000,
                affordability_override: false,
                discount: 0.1,
                proposal_prior: prior,
            };
            let mut state = super::super::Planning {
                outputs: [word(before[11], 0), word(before[11], 4)],
                ..Default::default()
            };
            let mut seen = vec![];
            let value = choose(
                &mut player,
                &mut profiles,
                &image,
                input,
                &mut state,
                &mut world,
                |call, _, _| {
                    seen.push(call.address);
                    Ok(())
                },
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(u32::from(value), expected, "{name} return");
            assert_eq!(seen, diagnostics, "{name} diagnostics");
            assert_eq!(identity, identity_after, "{name} identity");
            for (i, (a, b)) in player.bytes.iter().zip(after[0]).enumerate() {
                assert_eq!(a, b, "{name} player+{i:x}");
            }
            assert_eq!(profiles.concat(), after[1], "{name} profiles");
            assert_eq!(
                counts
                    .iter()
                    .flat_map(|x| x.to_le_bytes())
                    .collect::<Vec<_>>(),
                after[9],
                "{name} counts"
            );
            assert_eq!(
                components.map(f32::to_bits),
                std::array::from_fn(|i| word(after[10], i * 4)),
                "{name} components"
            );
            assert_eq!(
                state.outputs,
                [word(after[11], 0), word(after[11], 4)],
                "{name} outputs"
            );
            for i in 2..9 {
                assert_eq!(before[i], after[i], "{name} immutable{i}");
            }
        }
        assert!(r.0.is_empty());
    }
}

#[cfg(test)]
mod configured_parameter_tests {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/building_image.rs"
    ));

    #[test]
    fn ai_quote_and_demolition_receive_configured_resource() {
        let bytes = fixture();
        let image = PeImage::parse(&bytes).unwrap();
        let parameters = crate::building_parameters::BuildingKinds::from_parameters(
            "BuildingRent Retail 3\nBuildingCostPerUnit Retail 15\nBuildingCostToDemolish Retail 4",
            &image,
        )
        .unwrap();
        let mut categories = [[0; 380]; 52];
        let mut buildings = vec![[0; 916]; 600];
        let catalog = BusinessCatalog::from_rows([[0; 116]; 45]);
        let products = [[[0; 2]; 6]; 45];
        let commodities = [[0; 28]; 32];
        let residents = ResidentAccounts::new_pool();
        let player = Player {
            reference: 1,
            bytes: vec![],
            auxiliaries: vec![],
        };
        let mut counts = [0; 52 * 45];
        let mut identity = 0;
        let mut components = [0.0; 4];
        let world = World {
            building_parameters: parameters,
            categories: &categories,
            buildings: &buildings,
            catalog: &catalog,
            products: &products,
            commodities: &commodities,
            residents: &residents,
            player_references: &[1],
            player_count: 1,
            category_counts: &mut counts,
            proposal_identity: &mut identity,
            construction_components: &mut components,
            hour: 6,
            mode: 0,
            allowance: 0,
            affordability_override: false,
            discount: 0.0,
            proposal_prior: [0; 104],
        };
        let cost = quote(&[0, 2, 0, 0, 0, 0, 0, 0, 0, 0], &player, &world, &image).unwrap();
        assert_eq!(cost.total, 36);
        assert_eq!(
            cost.components.map(f32::to_bits),
            [30.0f32, 0.0, 6.0, 0.0].map(f32::to_bits)
        );
        // Release the borrowed AI proposal view before independent demolition.
        drop(world);
        categories[0][0x84..0x88].copy_from_slice(&2f32.to_le_bytes());
        buildings[0][4..8].copy_from_slice(&[0, 0, 1, 2]);
        buildings[0][8] = 2;
        buildings[0][0x40..0x44].copy_from_slice(&0x617628u32.to_le_bytes());
        let maintenance = crate::ai_maintenance::World {
            building_parameters: parameters,
            image: &image,
            profiles: &[],
            categories: &categories,
            buildings: &buildings,
            businesses: &[],
            catalog: &catalog,
            entries: &[],
            counts: &mut counts,
            identity: &mut identity,
            prior: [0; 104],
        };
        assert_eq!(
            crate::ai_maintenance::demolition_quote(0, &maintenance)
                .unwrap()
                .to_f32()
                .to_bits(),
            96f32.to_bits()
        );
    }
}
