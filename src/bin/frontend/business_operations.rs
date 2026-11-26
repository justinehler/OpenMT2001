//! Original building-panel producers over the authoritative worker session.
use crate::fresh_city::Session;
use openmt2001::{
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
    pub report: openmt2001::business_report::Report,
    pub graph: openmt2001::business_report::Graph,
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
        image: &openmt2001::pe::PeImage<'_>,
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
                openmt2001::ai_actions::liquidation_score::business_value(
                    business,
                    &city.buildings,
                    image,
                )?
                .to_f32(),
            ),
            Some(business_permissions::Trade::Takeover) => Some(
                openmt2001::business_takeover::price(
                    building,
                    &openmt2001::business_takeover::PriceWorld {
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
            Some(openmt2001::demolition::quote_with_parameters(row, &city.categories, &city.building_parameters, image)?.to_f32())
        } else {
            None
        };
        let mut conversions = Vec::new();
        for kind in image
            .at(0x53bdc4, 45 * 4)?
            .chunks_exact(4)
            .map(|b| word(b, 0))
        {
            let info = city
                .catalog
                .info
                .row(kind as usize)
                .ok_or("Conversion display order outside catalog")?;
            if !business_permissions::conversion_candidate(
                &target.inputs,
                kind,
                &city.catalog.info,
                openmt2001::business_conversion::fits(row, info)?,
            )? {
                continue;
            }
            let mut price = openmt2001::business_conversion::quote(
                building as u32,
                kind,
                &city.categories,
                &city.buildings,
                &city.catalog.info,
                image,
            )?
            .to_f32();
            if self.world.clock.hour() >= 6 {
                let volume = (i32::from(row[7]) - i32::from(row[5]) + 1)
                    .wrapping_mul(i32::from(row[6]) - i32::from(row[4]) + 1)
                    .wrapping_mul(i32::from(row[8]));
                let mut stock = business_controls::initial_stock(
                    kind as usize,
                    volume,
                    &city.catalog.info,
                    &city.catalog.products,
                    &city.commodities.rows,
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
        Ok(Inspection {
            graph: openmt2001::business_report::graph(
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
            base_value: openmt2001::ai_actions::liquidation_score::business_base_value(
                business,
                &city.buildings,
                image,
            )?
            .to_f32(),
            rent_age: openmt2001::business_report::rent_age(
                building,
                &city.buildings,
                &city.ledgers.businesses,
                &city.categories,
                self.world.clock.timing[1],
                image,
            )? as i32,
            report: openmt2001::business_report::inspect(
                &openmt2001::business_report::Inputs {
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
                let player = self.roster.manager.players.get(usize::from(business[10] - 1))
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
        image: &openmt2001::pe::PeImage<'_>,
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
                            openmt2001::business_conversion::fits(row, info)?,
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
        let mut submitted = false;
        business_controls::activate(action, &mut world, |call, _| {
            let command = match call {
                Call::Sale { business } => Some(live_commands::Command::SellBusiness(
                    openmt2001::business_sale::Command {
                        business,
                        player: owner.wrapping_sub(1),
                    },
                )),
                Call::Takeover { building } => Some(live_commands::Command::Takeover(
                    openmt2001::business_takeover::Command { building },
                )),
                Call::Demolish {
                    category,
                    building,
                    amount,
                } => Some(live_commands::Command::Demolish(
                    openmt2001::demolition::Command {
                        category: category as i8,
                        building: building as u16,
                        amount,
                    },
                )),
                Call::Convert { kind, building } => Some(live_commands::Command::Convert(
                    openmt2001::business_conversion::Command {
                        kind,
                        building,
                        owner: u32::from(owner),
                    },
                )),
                _ => None,
            };
            if let Some(command) = command {
                if network[0] && !network[1] {
                    return Err("building action requires network command transport".into());
                }
                self.live_commands
                    .submit_with_delay(command, tick, true, 0, |_| {
                        Err("building action delay wrapped into synchronous execution".into())
                    })?;
                submitted = true;
                Ok(())
            } else {
                presentation(call)
            }
        })?;
        Ok(submitted)
    }
}
