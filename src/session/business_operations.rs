//! Original building-panel producers over the authoritative worker session.
use crate::session::Session;
use crate::{
    business_controls::{self, Action, Call},
    business_permissions, live_commands,
};
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[derive(Clone, Debug)]
pub struct Conversion {
    pub kind: u32,
    pub label: u32,
    pub price: f32,
}
#[derive(Clone, Debug)]
pub struct Inspection {
    pub building: usize,
    pub business_label: u32,
    pub demolition_label: u32,
    pub owned: bool,
    pub owner_name: [u8; 96],
    pub owner_character: Option<u32>,
    pub report: crate::business_report::Report,
    pub graph: crate::business_report::Graph,
    pub base_value: f32,
    pub rent_age: i32,
    pub permissions: business_permissions::Permissions,
    pub trade_price: Option<f32>,
    pub demolition_price: Option<f32>,
    pub conversions: Vec<Conversion>,
}
struct Target {
    category: usize,
    player: usize,
    owner: u8,
    current: u32,
    references: Vec<u32>,
    inputs: business_permissions::Inputs,
}

struct ConversionWorld<'a> {
    categories: &'a [[u8; 380]; 52],
    buildings: &'a [[u8; 916]],
    catalog: &'a crate::business_pool::BusinessCatalog,
    products: &'a [[[u32; 2]; 6]; 45],
    commodities: &'a [[u8; 28]; 32],
}

/// Original 408140 fills both columns in the executable's display order.
/// Each row stores F(4453b0), then adds initial stock (4610d0) from 6am.
fn conversion_options(
    inputs: &business_permissions::Inputs,
    building: usize,
    world: ConversionWorld<'_>,
    hour: u32,
    image: &crate::pe::PeImage<'_>,
) -> Result<Vec<Conversion>, String> {
    let ConversionWorld {
        categories,
        buildings,
        catalog,
        products,
        commodities,
    } = world;
    let row = buildings
        .get(building)
        .ok_or("Conversion building outside pool")?;
    let mut conversions = Vec::new();
    for kind in image
        .at(0x53bdc4, 45 * 4)?
        .chunks_exact(4)
        .map(|b| word(b, 0))
    {
        let info = catalog
            .row(kind as usize)
            .ok_or("Conversion display order outside catalog")?;
        if !business_permissions::conversion_candidate(
            inputs,
            kind,
            catalog,
            crate::business_conversion::fits(row, info)?,
        )? {
            continue;
        }
        let mut price = crate::business_conversion::quote(
            building as u32,
            kind,
            categories,
            buildings,
            catalog,
            image,
        )?
        .to_f32();
        if hour >= 6 {
            let volume = (i32::from(row[7]) - i32::from(row[5]) + 1)
                .wrapping_mul(i32::from(row[6]) - i32::from(row[4]) + 1)
                .wrapping_mul(i32::from(row[8]));
            let mut stock = business_controls::initial_stock(
                kind as usize,
                volume,
                catalog,
                products,
                commodities,
            )?;
            stock.add_f32(price)?;
            price = stock.to_f32();
        }
        conversions.push(Conversion {
            kind,
            label: word(info, 8),
            price,
        });
    }
    Ok(conversions)
}

impl Session<'_> {
    fn business_operation_target(&self, building: usize) -> Result<Target, String> {
        let row = self
            .world
            .city
            .buildings
            .get(building)
            .ok_or("building action outside city")?;
        if row[0x3f] == 0 {
            return Err("selected building is no longer active".into());
        }
        let category_offset = word(row, 0x40)
            .checked_sub(0x617628)
            .ok_or("selected building category before pool")?;
        if !category_offset.is_multiple_of(380) {
            return Err("selected building category unaligned".into());
        }
        let category = self
            .world
            .city
            .categories
            .get((category_offset / 380) as usize)
            .ok_or("selected building category outside pool")?;
        let current = self.roster.status.current_player;
        let player = self
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == current)
            .ok_or("building action current player missing")?;
        let owner = self.roster.manager.players[player].bytes[0x1bc].wrapping_add(1);
        let business = self
            .world
            .city
            .ledgers
            .businesses
            .get(word(row, 0x1c) as usize)
            .ok_or("selected building has no live business")?;
        if business[0x21c] == 0 {
            return Err("selected business is no longer active".into());
        }
        let references: Vec<_> = self
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.reference)
            .collect();
        let resolve_owner = |owner: u8| -> Result<u32, String> {
            if owner == 0 {
                Ok(0)
            } else {
                references
                    .get((i32::from(owner as i8) - 1) as usize)
                    .copied()
                    .ok_or("building action owner outside roster".into())
            }
        };
        let inputs = business_permissions::Inputs {
            building_present: true,
            business_class: Some(word(business, 0)),
            business_kind: word(business, 4),
            business_owner: resolve_owner(business[10])?,
            category_owner: resolve_owner(category[5])?,
            current_player: current,
            category_reserved: category[0x6c],
            pending: row[0x3c..0x3f].try_into().unwrap(),
        };
        Ok(Target {
            category: (category_offset / 380) as usize,
            player,
            owner,
            current,
            references,
            inputs,
        })
    }
    pub fn inspect_business_operations(
        &self,
        building: usize,
        bounds: [f32; 4],
        image: &crate::pe::PeImage<'_>,
    ) -> Result<Inspection, String> {
        let target = self.business_operation_target(building)?;
        let permissions = business_permissions::evaluate(&target.inputs);
        let city = &self.world.city;
        let row = &city.buildings[building];
        let business = &city.ledgers.businesses[word(row, 0x1c) as usize];
        let owned = target.inputs.business_owner == target.current;
        // Original 407970 disables the trade and values tabs for class2.
        // Keep the existing values-tab quote for other classes even when a
        // separate pending-action gate temporarily disables trading.
        let trade_price = match (word(business, 0) != 2).then_some(if owned {
            business_permissions::Trade::Sell
        } else {
            business_permissions::Trade::Takeover
        }) {
            Some(business_permissions::Trade::Sell) => Some(
                crate::ai_actions::liquidation_score::business_value(
                    business,
                    &city.buildings,
                    image,
                )?
                .to_f32(),
            ),
            Some(business_permissions::Trade::Takeover) => Some(
                crate::business_takeover::price(
                    building,
                    &crate::business_takeover::PriceWorld {
                        categories: &city.categories,
                        buildings: &city.buildings,
                        businesses: &city.ledgers.businesses,
                        player_references: &target.references,
                    },
                    self.world.clock.timing[1],
                    image,
                )?
                .to_f32(),
            ),
            None => None,
        };
        let demolition_price = if permissions.demolish {
            Some(
                crate::demolition::quote_with_parameters(
                    row,
                    &city.categories,
                    &city.building_parameters,
                    image,
                )?
                .to_f32(),
            )
        } else {
            None
        };
        let conversions = conversion_options(
            &target.inputs,
            building,
            ConversionWorld {
                categories: &city.categories,
                buildings: &city.buildings,
                catalog: &city.catalog.info,
                products: &city.catalog.products,
                commodities: &city.commodities.rows,
            },
            self.world.clock.hour(),
            image,
        )?;
        Ok(Inspection {
            graph: crate::business_report::graph(
                business,
                [self.world.clock.calendar[1], self.world.clock.calendar[2]],
                bounds,
                image,
            )?,
            building,
            owned,
            demolition_label: word(
                city.catalog
                    .info
                    .row(word(business, 4) as usize)
                    .ok_or("Demolition label missing")?,
                4,
            ),
            base_value: crate::ai_actions::liquidation_score::business_base_value(
                business,
                &city.buildings,
                image,
            )?
            .to_f32(),
            rent_age: crate::business_report::rent_age(
                building,
                &city.buildings,
                &city.ledgers.businesses,
                &city.categories,
                self.world.clock.timing[1],
                image,
            )?,
            report: crate::business_report::inspect(
                &crate::business_report::Inputs {
                    business,
                    building: row,
                    category: &city.categories[target.category],
                    info: city
                        .catalog
                        .info
                        .row(word(business, 4) as usize)
                        .ok_or("Building report catalog missing")?,
                    products: city
                        .catalog
                        .products
                        .get(word(business, 4) as usize)
                        .ok_or("Building report products missing")?,
                    commodities: &city.commodities.rows,
                    owns_business_and_land: target.inputs.business_owner == target.current
                        && target.inputs.category_owner == target.current,
                    hour: self.world.clock.hour() as i32,
                },
                image,
            )?,
            business_label: word(
                city.catalog
                    .info
                    .row(word(business, 4) as usize)
                    .ok_or("Business label catalog missing")?,
                8,
            ),
            owner_name: if business[10] == 0 {
                [0; 96]
            } else {
                self.roster
                    .manager
                    .players
                    .get(usize::from(business[10] - 1))
                    .ok_or("Business owner missing")?
                    .bytes[0x158..0x1b8]
                    .try_into()
                    .unwrap()
            },
            owner_character: if business[10] == 0 {
                None
            } else {
                let player = self
                    .roster
                    .manager
                    .players
                    .get(usize::from(business[10] - 1))
                    .ok_or("Business owner missing")?;
                Some(word(&player.bytes, 0x150))
            },
            permissions,
            trade_price,
            demolition_price,
            conversions,
        })
    }
}

impl Session<'_> {
    /// Recheck the live selection before executing a UI action. The caller owns
    /// original presentation calls; financial values never come from the UI.
    pub fn business_operation(
        &mut self,
        building: usize,
        action: Action,
        image: &crate::pe::PeImage<'_>,
        network: [bool; 2],
        mut presentation: impl FnMut(Call) -> Result<(), String>,
    ) -> Result<bool, String> {
        let target = self.business_operation_target(building)?;
        let gates = target.inputs;
        let current = target.current;
        let owner = target.owner;
        let player = target.player;
        let references = target.references;
        let row = &self.world.city.buildings[building];
        let category = &self.world.city.categories[target.category];
        let allowed = business_permissions::evaluate(&gates);
        let permitted = match action {
            Action::Trade => allowed.trade.is_some(),
            Action::Demolish => allowed.demolish,
            Action::Convert { selected_label } => {
                let mut candidate = false;
                for (kind, info) in self.world.catalog.info.rows().iter().enumerate() {
                    if word(info, 8) == selected_label
                        && business_permissions::conversion_candidate(
                            &gates,
                            kind as u32,
                            &self.world.catalog.info,
                            crate::business_conversion::fits(row, info)?,
                        )?
                    {
                        candidate = true;
                        break;
                    }
                }
                candidate
            }
        };
        if !permitted {
            return Err("building action is no longer enabled by the original panel rules".into());
        }
        let tick = self.world.clock.timing[0];
        let mut world = business_controls::World {
            building_parameters: self.world.city.building_parameters,
            image,
            selected: Some(building),
            selected_category: word(category, 0) as i32,
            current_player: current,
            player_references: &references,
            cash: self.world.city.ledgers.players[player].cash,
            allowance: i32::from_le_bytes(image.at(0x541c7c, 4)?.try_into().unwrap()),
            affordability_override: false,
            hour: self.world.clock.hour() as i32,
            ownership_tick: self.world.clock.timing[1],
            categories: &self.world.city.categories,
            buildings: &mut self.world.city.buildings,
            businesses: &self.world.city.ledgers.businesses,
            catalog: &self.world.city.catalog.info,
            products: &self.world.city.catalog.products,
            commodities: &self.world.city.commodities.rows,
        };
        let mut commands = Vec::new();
        let mut submitted = false;
        business_controls::activate(action, &mut world, |call, _| {
            let command = match call {
                Call::Sale { business } => Some(live_commands::Command::SellBusiness(
                    crate::business_sale::Command {
                        business,
                        player: owner.wrapping_sub(1),
                    },
                )),
                Call::Takeover { building } => Some(live_commands::Command::Takeover(
                    crate::business_takeover::Command { building },
                )),
                Call::Demolish {
                    category,
                    building,
                    amount,
                } => Some(live_commands::Command::Demolish(
                    crate::demolition::Command {
                        category: category as i8,
                        building: building as u16,
                        amount,
                    },
                )),
                Call::Convert { kind, building } => Some(live_commands::Command::Convert(
                    crate::business_conversion::Command {
                        kind,
                        building,
                        owner: u32::from(owner),
                    },
                )),
                _ => None,
            };
            if let Some(command) = command {
                if self.connected.is_some() {
                    commands.push(command);
                } else {
                    if network[0] && !network[1] {
                        return Err("building action requires network command transport".into());
                    }
                    self.live_commands
                        .submit_with_delay(command, tick, true, 0, |_| {
                            Err("building action delay wrapped into synchronous execution".into())
                        })?;
                }
                submitted = true;
                Ok(())
            } else {
                presentation(call)
            }
        })?;
        for command in commands {
            if let Some(connection) = &mut self.connected {
                let targets = crate::network::commands::Targets::new(
                    &self.world.city.buildings,
                    &self.world.city.ledgers.businesses,
                    &self.world.city.categories,
                );
                connection.commands.submit(
                    &connection.worker,
                    command,
                    &mut self.live_commands,
                    &mut self.world.city.categories,
                    targets,
                    tick,
                )?;
            }
        }
        Ok(submitted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{business_pool::BusinessCatalog, pe::PeImage, player_manager::put};

    fn image_bytes() -> Vec<u8> {
        let mut bytes = vec![0; 0x20100];
        bytes[..2].copy_from_slice(b"MZ");
        put(&mut bytes, 0x3c, 0x40);
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes[0x44..0x46].copy_from_slice(&0x14cu16.to_le_bytes());
        bytes[0x46..0x48].copy_from_slice(&1u16.to_le_bytes());
        bytes[0x54..0x56].copy_from_slice(&32u16.to_le_bytes());
        bytes[0x58..0x5a].copy_from_slice(&0x10bu16.to_le_bytes());
        put(&mut bytes, 0x74, 0x400000);
        put(&mut bytes, 0x84, 0x120000);
        put(&mut bytes, 0x88, 0x20000);
        put(&mut bytes, 0x8c, 0x100);
        for (address, value) in [
            (0x52502c, 0.5f32),
            (0x525034, 1.),
            (0x5251e8, 1.5),
            (0x525b10, 1.2),
            (0x525c50, 0.4),
        ] {
            put(&mut bytes, address - 0x520000 + 0x100, value.to_bits());
        }
        for (address, value) in [(0x525b00, 0.8f64), (0x525b08, 0.6), (0x5250e8, 0.5)] {
            let at = address - 0x520000 + 0x100;
            bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        // Deliberately nonnumeric order, with inadmissible candidates between.
        for (i, kind) in [0, 3, 42, 2, 43, 4].into_iter().enumerate() {
            put(&mut bytes, 0x53bdc4 - 0x520000 + 0x100 + i * 4, kind);
        }
        bytes[0x5365f4 - 0x520000 + 0x100..0x5365f6 - 0x520000 + 0x100]
            .copy_from_slice(&4u16.to_le_bytes());
        bytes
    }
    fn gates() -> business_permissions::Inputs {
        business_permissions::Inputs {
            building_present: true,
            business_class: Some(0),
            business_kind: 0,
            business_owner: 1,
            category_owner: 0,
            current_player: 1,
            category_reserved: 0,
            pending: [0; 3],
        }
    }
    #[test]
    fn production_conversion_list_keeps_display_order_fit_and_daytime_prices() {
        let bytes = image_bytes();
        let image = PeImage::parse(&bytes).unwrap();
        let mut buildings = [[0; 916]; 1];
        buildings[0][6] = 2; // 3 x 2 x 2 = 12 units
        buildings[0][7] = 1;
        buildings[0][8] = 2;
        put(&mut buildings[0], 0x40, 0x617628);
        let mut categories = [[0; 380]; 52];
        put(&mut categories[0], 0x84, 5f32.to_bits());
        let mut rows = [[0; 116]; 45];
        for (kind, cost, class) in [
            (0, 99, 0),
            (2, 20, 0),
            (3, 10, 0),
            (42, 30, 1),
            (43, 40, 0),
            (4, 50, 0),
        ] {
            let row = &mut rows[kind];
            put(row, 0, kind as u32);
            put(row, 8, 100 + kind as u32);
            put(row, 12, class);
            row[0x30] = 1;
            put(row, 0x38, 1);
            put(row, 0x3c, if kind == 4 { 5 } else { 20 });
            put(row, 0x40, 7);
            put(row, 0x44, 3);
            put(row, 0x48, 1);
            row[0x4c] = 1;
            put(row, 0x54, cost);
            put(row, 0x58, 2f32.to_bits());
        }
        let catalog = BusinessCatalog::from_rows(rows);
        let products = [[[0, 1f32.to_bits()]; 6]; 45];
        let mut commodities = [[0; 28]; 32];
        put(&mut commodities[0], 0x14, 3f32.to_bits());
        let options = |inputs: &business_permissions::Inputs, hour| {
            conversion_options(
                inputs,
                0,
                ConversionWorld {
                    categories: &categories,
                    buildings: &buildings,
                    catalog: &catalog,
                    products: &products,
                    commodities: &commodities,
                },
                hour,
                &image,
            )
            .unwrap()
        };
        let before = options(&gates(), 5);
        assert_eq!(
            before
                .iter()
                .map(|r| (r.kind, r.label, r.price))
                .collect::<Vec<_>>(),
            [(3, 103, 280.), (2, 102, 560.)]
        );
        // ((12*.6+.8)*(5*.5+1)*refurbcost)*quality, plus 12*2*3 stock.
        let after = options(&gates(), 6);
        assert_eq!(
            after.iter().map(|r| (r.kind, r.price)).collect::<Vec<_>>(),
            [(3, 352.), (2, 632.)]
        );
        for hour in [12, 23] {
            assert_eq!(
                options(&gates(), hour)
                    .iter()
                    .map(|r| r.price)
                    .collect::<Vec<_>>(),
                [352., 632.]
            );
        }
        let mut pending = gates();
        pending.pending[1] = 1;
        assert!(options(&pending, 6).is_empty());
    }
    #[test]
    fn panel_value_inputs_follow_original_capital_lease_sale_and_demolition_formulas() {
        let bytes = image_bytes();
        let image = PeImage::parse(&bytes).unwrap();
        let mut building = [0; 916];
        building[6] = 2;
        building[7] = 1;
        building[8] = 2;
        put(&mut building, 0x38, 1000f32.to_bits());
        put(&mut building, 0x40, 0x617628);
        let buildings = [building];
        let mut business = [0; 544];
        put(&mut business, 0x20, 50f32.to_bits());
        put(&mut business, 0x64, 100f32.to_bits()); // previous period when bank=0
        let categories = [[0; 380]; 52];
        assert_eq!(
            crate::ai_actions::liquidation_score::business_base_value(
                &business, &buildings, &image
            )
            .unwrap()
            .to_f32(),
            1125.
        );
        assert_eq!(
            crate::ai_actions::liquidation_score::business_value(&business, &buildings, &image)
                .unwrap()
                .to_f32(),
            450.
        );
        assert_eq!(
            crate::business_report::rent_age(
                0,
                &buildings,
                &[business],
                &categories,
                59 * 625,
                &image
            )
            .unwrap(),
            41
        );
        let mut categories = categories;
        put(&mut categories[0], 0x84, 5f32.to_bits());
        let parameters = crate::building_parameters::BuildingKinds::from_image(&image).unwrap();
        assert_eq!(
            crate::demolition::quote_with_parameters(&building, &categories, &parameters, &image)
                .unwrap()
                .to_f32(),
            168.
        );
    }
}
