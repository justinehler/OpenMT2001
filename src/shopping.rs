//! Runtime and saved-world bindings for the recovered resident shopping action.
//! Runtime business/account records stay with the caller; topology holds indices.

use crate::{board::Board, resident::ShoppingNode, save::SaveFile};

/// Ordered transient catalogs reconstructed from literal original settings.
/// Save masks and counts cannot recover order for multi-product businesses.
pub struct Catalogs {
    pub commodities: [Vec<u32>; 45],
    pub fractions: [Vec<f32>; 45],
}

pub(crate) fn business_names<'a>(
    image: &crate::pe::PeImage<'a>,
) -> Result<std::collections::BTreeMap<&'a str, usize>, String> {
    let mut names = std::collections::BTreeMap::new();
    let mut at = 0x4d90a2;
    // Read the actual enum registrations, including their float ID operands.
    for id in 0..45 {
        let push = image.at(at, 5)?;
        if push[0] != 0x68 {
            return Err("unsupported business enum registration".into());
        }
        let address = u32::from_le_bytes(push[1..].try_into().unwrap());
        let bytes = image.at(address, 64)?;
        let end = bytes
            .iter()
            .position(|b| *b == 0)
            .ok_or("unterminated business name")?;
        let name = std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid business name")?;
        at += 5;
        let value = image.at(at, 5)?;
        let bits = if value[0] == 0x6a {
            at += 2;
            value[1] as u32
        } else if value[0] == 0x68 {
            at += 5;
            u32::from_le_bytes(value[1..].try_into().unwrap())
        } else {
            return Err("unsupported business enum value".into());
        };
        if f32::from_bits(bits) != id as f32 {
            return Err("nonsequential business enum".into());
        }
        let call = image.at(at, 10)?;
        if call[..6] != [0xb9, 0xb0, 0xaa, 0x93, 0, 0xe8] {
            return Err("unsupported business enum binding".into());
        }
        let destination = at
            .wrapping_add(10)
            .wrapping_add(u32::from_le_bytes(call[6..10].try_into().unwrap()));
        if destination != 0x4d64a0 {
            return Err("unexpected business enum binder".into());
        }
        at += 10;
        if names.insert(name, id).is_some() {
            return Err("duplicate business enum name".into());
        }
    }
    Ok(names)
}

impl Catalogs {
    pub fn from_v14_defaults(source: &str, image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let names = business_names(image)?;
        let commodities = crate::resident::commodity_names(image)?;
        let scale = f64::from_le_bytes(image.at(0x5267e0, 8)?.try_into().unwrap());
        let mut result = Self {
            commodities: std::array::from_fn(|_| Vec::new()),
            fractions: std::array::from_fn(|_| Vec::new()),
        };
        for line in source.lines() {
            let line = line.split("--").next().unwrap().trim();
            if !line.contains(":AddCommodity(") {
                continue;
            }
            let (name, values) = line
                .strip_prefix("BusinessInfo[")
                .and_then(|s| s.strip_suffix(");"))
                .and_then(|s| s.split_once("]:AddCommodity("))
                .ok_or("non-literal catalog statement")?;
            let kind = *names
                .get(name.trim())
                .ok_or("unknown catalog business name")?;
            let (commodity, percentage) =
                values.split_once(',').ok_or("missing catalog percentage")?;
            let commodity = u32::from(
                *commodities
                    .get(commodity.trim())
                    .ok_or("unknown catalog commodity")?,
            );
            let fraction = (percentage
                .trim()
                .parse::<f64>()
                .map_err(|_| "non-literal catalog percentage")?
                * scale) as f32;
            if !fraction.is_finite() {
                return Err("non-finite catalog percentage".into());
            }
            // Native AddCommodity ignores duplicate identities and a full table.
            if result.commodities[kind].len() == 6 || result.commodities[kind].contains(&commodity)
            {
                continue;
            }
            result.commodities[kind].push(commodity);
            result.fractions[kind].push(fraction);
        }
        if result.commodities.iter().all(Vec::is_empty) {
            return Err("missing catalog statements".into());
        }
        Ok(result)
    }

    pub fn validate_saved(&self, economy: &crate::save::SavedEconomy<'_>) -> Result<(), String> {
        for (kind, catalog) in self.commodities.iter().enumerate() {
            let mask = catalog
                .iter()
                .fold(0_u32, |mask, id| mask | 1_u32.wrapping_shl(*id));
            if economy.commodity_count(kind as u32) != Some(catalog.len() as u8)
                || economy.commodity_mask(kind as u32) != Some(mask)
            {
                return Err(format!(
                    "saved catalog{kind} differs from supplied script context"
                ));
            }
        }
        Ok(())
    }
}

/// Settings restored from the save plus the matching literal script context.
pub struct Settings {
    pub catalogs: Catalogs,
    pub preferences: crate::resident::Preferences,
    pub commodities: [crate::resident::Commodity; 32],
    pub weights: [[f32; 4]; 3],
    pub business_distance_weights: [f32; 3],
    pub interaction_delays: [f32; 45],
    pub unit_costs: [f32; 32],
}

impl Settings {
    /// Snapshot script-final settings for a simulation batch. Rebuild after
    /// script mutations; source parsing cannot recover live catalog order.
    #[cfg(feature = "lua4")]
    pub fn from_runtime(
        catalog: &crate::business_script::State,
        commodities: &crate::commodity_script::State,
        configuration: &crate::resident::Configuration,
    ) -> Result<Self, String> {
        let mut catalogs = Catalogs {
            commodities: std::array::from_fn(|_| Vec::new()),
            fractions: std::array::from_fn(|_| Vec::new()),
        };
        let mut interaction_delays = [0.; 45];
        for (kind, delay) in interaction_delays.iter_mut().enumerate() {
            let row = catalog
                .info
                .row(kind)
                .ok_or("missing runtime business info")?;
            let products = catalog.products[kind]
                .get(..usize::from(row[0x4c]))
                .ok_or("runtime catalog exceeds six products")?;
            for &[identity, fraction] in products {
                if identity >= 32 {
                    return Err("runtime catalog commodity exceeds table".into());
                }
                catalogs.commodities[kind].push(identity);
                catalogs.fractions[kind].push(f32::from_bits(fraction));
            }
            *delay = f32::from_le_bytes(row[0x50..0x54].try_into().unwrap());
        }
        let commodities = commodities.resident_inputs();
        Ok(Self {
            catalogs,
            preferences: configuration.preferences.clone(),
            commodities,
            weights: configuration
                .class_weights
                .rows
                .map(|r| [r[0], r[1], r[2], r[3]]),
            business_distance_weights: configuration.class_weights.rows.map(|r| r[4]),
            interaction_delays,
            unit_costs: commodities.map(|c| c.value),
        })
    }

    pub fn from_saved(
        save: &SaveFile<'_>,
        image: &crate::pe::PeImage<'_>,
        defaults: &str,
        business_defaults: &str,
        parameters: &str,
    ) -> Result<Self, String> {
        let economy = save.economy()?;
        let catalogs = Catalogs::from_v14_defaults(business_defaults, image)?;
        catalogs.validate_saved(&economy)?;
        let class = crate::resident::ClassWeights::from_parameters(parameters, image)?;
        let commodities = economy.resident_commodities();
        Ok(Self {
            catalogs,
            preferences: crate::resident::Preferences::from_v14_defaults(defaults, image)?,
            weights: class.rows.map(|r| [r[0], r[1], r[2], r[3]]),
            business_distance_weights: class.rows.map(|r| r[4]),
            unit_costs: commodities.map(|c| c.value),
            interaction_delays: economy.interaction_delays(),
            commodities,
        })
    }
}

/// Mutable ledgers and explicit scheduler inputs; no serialized pointers are
/// followed, and this binding does not invent animation-manager or traffic ticks.
pub struct Runtime<'a> {
    pub businesses: &'a mut [[u8; 544]],
    pub accounts: &'a mut [crate::resident::SpendingAccount],
    pub owners: &'a mut [crate::economy::PlayerAccount],
    pub routes: &'a mut crate::routing::CategoryRoutes,
    pub lanes: &'a mut [[u8; 32]],
    pub tick: u32,
    pub boundary: i32,
    pub distance_disabled: bool,
}

pub struct Session<'a, 'w> {
    pub topology: &'a mut Topology,
    pub settings: &'a Settings,
    pub runtime: Runtime<'w>,
}

/// Explicit live model/auxiliary bindings. These are resolved runtime handles,
/// never pointers copied from a save. Keep slot identity when a resident moves.
pub struct ResidentBinding {
    pub resident_reference: u32,
    pub auxiliary_reference: u32,
    pub auxiliary_slot: usize,
    pub model_frame_count: u16,
}

/// Shared population resources for the scheduler's separate Residents and
/// Animations events. All transactions use the caller's actual ledger records.
pub struct Population<'a> {
    pub topology: &'a mut Topology,
    pub settings: &'a Settings,
    pub routes: &'a mut crate::routing::CategoryRoutes,
    pub lanes: &'a mut [[u8; 32]],
    pub auxiliary: &'a mut crate::resident::AuxiliaryPool,
    pub animations: &'a mut crate::resident::ResidentAnimationPool,
    pub animation_settings: &'a crate::resident::ResidentAnimationSettings,
    pub bindings: &'a [ResidentBinding],
}

/// Borrow the authoritative pools from either a fresh world or loaded ledgers.
/// No resident, business or player record is copied to run a population pass.
pub struct PopulationRecords<'a> {
    pub residents: &'a mut crate::economy::ResidentAccounts,
    pub businesses: &'a mut [[u8; 0x220]],
    pub players: &'a mut [crate::economy::PlayerAccount],
}

impl<'a> From<&'a mut crate::economy::Ledgers> for PopulationRecords<'a> {
    fn from(ledgers: &'a mut crate::economy::Ledgers) -> Self {
        Self {
            residents: &mut ledgers.residents,
            businesses: &mut ledgers.businesses,
            players: &mut ledgers.players,
        }
    }
}

/// Additional shared inputs for housing, home and roaming actions. Business,
/// resident and player records remain in the same ledgers as retail actions.
pub struct CityActions<'a> {
    pub demand: Option<&'a mut crate::ai_demand::State>,
    pub housing: &'a crate::housing::Topology,
    pub housing_settings: &'a crate::housing::Settings,
    pub welfare: &'a crate::resident::Welfare,
    pub objectives: &'a crate::resident::DailyObjectives,
    pub objective_rng: &'a mut u32,
    pub constructor_rng: &'a mut [u32; 3],
    pub available: &'a mut Vec<usize>,
    pub category_enabled: &'a [u8; 52],
    pub properties: &'a mut crate::economy::PropertyIncome,
}

impl Population<'_> {
    pub fn tick(
        &mut self,
        ledgers: &mut crate::economy::Ledgers,
        inputs: PopulationTick,
        tick_helper: impl FnMut(
            usize,
            crate::resident::TickEvent,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
        other: impl FnMut(
            usize,
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick_inner(ledgers.into(), inputs, None, tick_helper, other)
    }

    pub fn tick_with_city_actions(
        &mut self,
        ledgers: &mut crate::economy::Ledgers,
        inputs: PopulationTick,
        city: &mut CityActions<'_>,
        tick_helper: impl FnMut(
            usize,
            crate::resident::TickEvent,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
        other: impl FnMut(
            usize,
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick_with_city_records(ledgers.into(), inputs, city, tick_helper, other)
    }

    pub fn tick_with_city_records(
        &mut self,
        records: PopulationRecords<'_>,
        inputs: PopulationTick,
        city: &mut CityActions<'_>,
        tick_helper: impl FnMut(
            usize,
            crate::resident::TickEvent,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
        other: impl FnMut(
            usize,
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick_inner(records, inputs, Some(city), tick_helper, other)
    }

    fn tick_inner(
        &mut self,
        ledgers: PopulationRecords<'_>,
        inputs: PopulationTick,
        mut city: Option<&mut CityActions<'_>>,
        mut tick_helper: impl FnMut(
            usize,
            crate::resident::TickEvent,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
        mut other: impl FnMut(
            usize,
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        ledgers.residents.advance_population(|index, residents| {
            let binding = self
                .bindings
                .get(index)
                .ok_or("population resident has no runtime binding")?;
            if residents.nodes[index].auxiliary_reference() != binding.auxiliary_reference {
                return Err("population auxiliary binding does not match resident".into());
            }
            let auxiliary = self
                .auxiliary
                .get_mut(binding.auxiliary_slot)
                .ok_or("population auxiliary leaves allocated pool")?;
            let centers = self.topology.geometry.centers;
            residents.tick_resident(
                index,
                auxiliary,
                &centers,
                |event, index, residents, auxiliary| {
                    if event == crate::resident::TickEvent::Movement {
                        return Ok(residents.nodes[index].advance_movement(auxiliary));
                    }
                    if event != crate::resident::TickEvent::Action {
                        return tick_helper(index, event, &mut residents.nodes[index], auxiliary);
                    }
                    let action = u32::from_le_bytes(
                        residents.nodes[index].as_bytes()[0xcc..0xd0]
                            .try_into()
                            .unwrap(),
                    );
                    if let Some(city) = city.as_deref_mut()
                        && matches!(action, 0 | 1 | 12)
                    {
                        let mut animate =
                            |action,
                             resident: &mut crate::resident::Resident,
                             auxiliary: &mut [u8; 284]| {
                                resident.request_animation(
                                    auxiliary,
                                    self.animations,
                                    self.animation_settings,
                                    crate::resident::ResidentAnimationRequest {
                                        action,
                                        resident_reference: binding.resident_reference,
                                        auxiliary_reference: binding.auxiliary_reference,
                                        model_frame_count: binding.model_frame_count,
                                    },
                                )
                            };
                        let mut crossing = crate::resident::CrossingWorld {
                            category_links: &self.topology.crossing.category_links,
                            road_neighbors: &self.topology.crossing.road_neighbors,
                            lane_base: self.topology.crossing.lane_base,
                            lanes: self.lanes,
                        };
                        let mut travel = crate::resident::CategoryTravelState {
                            centers: &centers,
                            boundary: inputs.boundary,
                            category_counts: &mut self.topology.category_counts,
                            routes: self.routes,
                            crossing: &mut crossing,
                        };
                        if action == 0 {
                            let mut average =
                                |businesses: &[[u8; 0x220]]| city.housing.average_rent(businesses);
                            let mut housing = crate::housing::World {
                                demand: city.demand.as_deref_mut(),
                                buildings: &city.housing.buildings,
                                categories: &city.housing.categories,
                                category_values: &self.topology.category_factors,
                                businesses: ledgers.businesses,
                                available: city.available,
                                players: ledgers.players,
                                welfare: city.welfare,
                                settings: city.housing_settings,
                                average_rent: &mut average,
                            };
                            crate::housing::advance_dispatched_action(
                                residents,
                                index,
                                auxiliary,
                                &mut housing,
                                city.objectives,
                                city.objective_rng,
                                |helper, resident, auxiliary| match helper {
                                    crate::resident::HousingAction::TravelA => resident
                                        .advance_category_route(
                                            auxiliary,
                                            &mut travel,
                                            &mut animate,
                                        ),
                                    crate::resident::HousingAction::TravelB => resident
                                        .advance_fallback_travel(
                                            auxiliary,
                                            travel.boundary,
                                            travel.centers,
                                            travel.category_counts,
                                        ),
                                    _ => Err("unexpected deep housing operation".into()),
                                },
                            )?;
                        } else {
                            residents.nodes[index].advance_home_and_roaming_action(
                                auxiliary,
                                &mut crate::resident::RoamingState {
                                    travel,
                                    category_enabled: city.category_enabled,
                                    rng: city.constructor_rng,
                                },
                                &mut animate,
                                |helper, resident, auxiliary| {
                                    other(index, helper, resident, auxiliary)
                                },
                                city.properties,
                            )?;
                        }
                        return Ok(false);
                    }
                    Session {
                        topology: self.topology,
                        settings: self.settings,
                        runtime: Runtime {
                            businesses: ledgers.businesses,
                            accounts: &mut residents.accounts,
                            owners: ledgers.players,
                            routes: self.routes,
                            lanes: self.lanes,
                            tick: inputs.tick,
                            boundary: inputs.boundary,
                            distance_disabled: inputs.distance_disabled,
                        },
                    }
                    .advance_action(
                        &mut residents.nodes[index],
                        auxiliary,
                        |action, resident, auxiliary| {
                            resident.request_animation(
                                auxiliary,
                                self.animations,
                                self.animation_settings,
                                crate::resident::ResidentAnimationRequest {
                                    action,
                                    resident_reference: binding.resident_reference,
                                    auxiliary_reference: binding.auxiliary_reference,
                                    model_frame_count: binding.model_frame_count,
                                },
                            )
                        },
                        |event, resident, auxiliary| other(index, event, resident, auxiliary),
                    )?;
                    Ok(false)
                },
            )
        })
    }

    /// Resolve each controller through its auxiliary handle, not resident
    /// iteration order. Non-resident controllers remain an explicit callout.
    pub fn advance_animations(
        &mut self,
        residents: &mut crate::economy::ResidentAccounts,
        mut other: impl FnMut(
            usize,
            &mut crate::resident::ResidentAnimationPool,
            &mut crate::resident::AuxiliaryPool,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.animations.advance(|index, pool| {
            let record = &pool.records()[index];
            let kind = u32::from_le_bytes(record[..4].try_into().unwrap());
            let reference = u32::from_le_bytes(record[8..12].try_into().unwrap());
            if kind <= 3
                && let Some((slot, binding)) = self
                    .bindings
                    .iter()
                    .enumerate()
                    .find(|(_, b)| b.auxiliary_reference == reference)
            {
                let node = residents
                    .nodes
                    .get_mut(slot)
                    .ok_or("animation resident leaves pool")?;
                if node.auxiliary_reference() != reference {
                    return Err("animation auxiliary binding does not match resident".into());
                }
                let auxiliary = self
                    .auxiliary
                    .get_mut(binding.auxiliary_slot)
                    .ok_or("animation auxiliary leaves allocated pool")?;
                node.advance_animation_slot(
                    auxiliary,
                    pool,
                    index,
                    binding.resident_reference,
                    reference,
                )
            } else {
                other(index, pool, self.auxiliary)
            }
        })
    }
}

pub struct PopulationTick {
    pub tick: u32,
    pub boundary: i32,
    pub distance_disabled: bool,
}

impl Session<'_, '_> {
    pub fn tick(
        &mut self,
        resident: &mut crate::resident::Resident,
        auxiliary: &mut [u8; 284],
        animation: impl FnMut(u32, &mut crate::resident::Resident, &mut [u8; 284]) -> Result<(), String>,
        tick_helper: impl FnMut(
            crate::resident::TickEvent,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
        other: impl FnMut(
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.run(false, resident, auxiliary, animation, tick_helper, other)
    }

    /// Dispatch actions after the authoritative pool has run tick gates.
    pub fn advance_action(
        &mut self,
        resident: &mut crate::resident::Resident,
        auxiliary: &mut [u8; 284],
        animation: impl FnMut(u32, &mut crate::resident::Resident, &mut [u8; 284]) -> Result<(), String>,
        other: impl FnMut(
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.run(
            true,
            resident,
            auxiliary,
            animation,
            |_, _, _| Err("action dispatch invoked tick gate".into()),
            other,
        )
    }

    fn run(
        &mut self,
        action_only: bool,
        resident: &mut crate::resident::Resident,
        auxiliary: &mut [u8; 284],
        animation: impl FnMut(u32, &mut crate::resident::Resident, &mut [u8; 284]) -> Result<(), String>,
        tick_helper: impl FnMut(
            crate::resident::TickEvent,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
        other: impl FnMut(
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        let class = resident.class() as usize;
        let category_owners = self.topology.category_owners();
        let distance_weight = *self
            .settings
            .business_distance_weights
            .get(class)
            .ok_or("shopping class outside settings")?;
        let retail = crate::resident::RetailWorld {
            building_businesses: &self.topology.building_businesses,
            catalogs: &self.settings.catalogs.commodities,
            interaction_delays: &self.settings.interaction_delays,
            unit_costs: &self.settings.unit_costs,
            preferences: &self.settings.preferences,
            tick: self.runtime.tick,
        };
        let mut state = crate::resident::ShoppingState {
            local: crate::resident::LocalPurchaseState {
                retail: &retail,
                centers: &self.topology.geometry.centers,
                boundary: self.runtime.boundary,
                category_counts: &mut self.topology.category_counts,
                businesses: self.runtime.businesses,
                accounts: self.runtime.accounts,
                owners: self.runtime.owners,
            },
            selection: crate::resident::ShoppingSelection {
                chains: &self.topology.chains,
                nodes: &self.topology.nodes,
                category_masks: &self.topology.category_masks,
                category_factors: &self.topology.category_factors,
                building_categories: &self.topology.building_categories,
                building_attributes: &self.topology.building_attributes,
                commodities: &self.settings.commodities,
                weights: &self.settings.weights,
                distance_disabled: self.runtime.distance_disabled,
                category_owners: &category_owners,
                entrances: &self.topology.entrances,
                business_distance_weight: distance_weight,
            },
            routes: self.runtime.routes,
            crossing: &mut crate::resident::CrossingWorld {
                category_links: &self.topology.crossing.category_links,
                road_neighbors: &self.topology.crossing.road_neighbors,
                lane_base: self.topology.crossing.lane_base,
                lanes: self.runtime.lanes,
            },
        };
        if action_only {
            resident.advance_shopping_action(auxiliary, &mut state, animation, other)
        } else {
            resident.tick_with_shopping(auxiliary, &mut state, animation, tick_helper, other)
        }
    }
}

pub struct Topology {
    pub geometry: crate::board::CategoryGeometry,
    pub crossing: crate::board::CrossingTopology,
    /// Enabled mapping consumed by the transaction; selection's resolver does
    /// not test the enabled byte and uses the category chains below.
    pub building_businesses: Vec<Option<usize>>,
    pub building_categories: Vec<i16>,
    pub building_attributes: Vec<i8>,
    pub entrances: Vec<[f32; 3]>,
    pub chains: [Vec<Option<usize>>; 52],
    pub nodes: [Vec<ShoppingNode>; 52],
    pub category_masks: [u32; 52],
    pub category_factors: [f32; 52],
    /// Shared ownership and hourly countdowns; shopping derives current owners
    /// from these records so expiry cannot leave a stale owner cache.
    pub categories: [crate::category::State; 52],
    pub category_counts: [u32; 52],
}

impl Topology {
    /// Publish the counters changed by resident routing to the live city rows.
    /// Call before script/category work or rebuilding this derived topology.
    /// Ownership and countdowns are managed separately and are not overwritten.
    pub fn write_resident_counts(&self, categories: &mut [[u8; 0x17c]; 52]) {
        for (row, count) in categories.iter_mut().zip(self.category_counts) {
            row[0x88..0x8c].copy_from_slice(&count.to_le_bytes());
        }
    }

    /// Publish only ownership/countdown fields owned by the hourly category
    /// pass. Preserve current script flags, masks, prestige and resident counts.
    pub fn write_category_ownership(&self, categories: &mut [[u8; 0x17c]; 52]) {
        for (row, state) in categories.iter_mut().zip(self.categories) {
            row[5] = state.owner;
            row[0x78..0x7c].copy_from_slice(&state.ownership_state.to_le_bytes());
            row[0x7c..0x80].copy_from_slice(&state.countdowns[0].to_le_bytes());
            row[0x80..0x84].copy_from_slice(&state.countdowns[1].to_le_bytes());
        }
    }

    /// Project the live city after business attachment and category refresh.
    /// Rebuild after topology changes. Mutable category fields are a snapshot;
    /// the caller must share/write back their updates before another projection.
    pub fn from_runtime(
        records: &[[u8; 0x394]],
        categories: &[[u8; 0x17c]; 52],
        building_base: u32,
        board: &Board,
    ) -> Result<Self, String> {
        let word = |row: &[u8], at| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
        let chains = crate::building_pool::category_chains(categories, records, building_base)?;
        let notifications = board.category_notification_values();
        let mut businesses = Vec::with_capacity(records.len());
        let mut result = Self {
            geometry: board.category_geometry(),
            crossing: board.crossing_topology()?,
            building_businesses: Vec::with_capacity(records.len()),
            building_categories: vec![-1; records.len()],
            building_attributes: Vec::with_capacity(records.len()),
            entrances: Vec::with_capacity(records.len()),
            chains: std::array::from_fn(|_| Vec::new()),
            nodes: std::array::from_fn(|_| Vec::new()),
            category_masks: std::array::from_fn(|i| word(&categories[i], 0x70)),
            category_factors: std::array::from_fn(|i| f32::from_bits(word(&categories[i], 0x84))),
            categories: std::array::from_fn(|i| crate::category::State {
                owner: categories[i][5],
                ownership_state: word(&categories[i], 0x78),
                countdowns: [word(&categories[i], 0x7c), word(&categories[i], 0x80)],
                notification_value: notifications[i],
            }),
            category_counts: std::array::from_fn(|i| word(&categories[i], 0x88)),
        };
        for row in records {
            let index = word(row, 0x1c) as i32;
            let business = if index == -1 {
                None
            } else {
                let index =
                    usize::try_from(index).map_err(|_| "negative runtime business index")?;
                if index >= 600 {
                    return Err("runtime business index exceeds pool".into());
                }
                Some(index)
            };
            businesses.push(business);
            result
                .building_businesses
                .push(if row[0x3f] != 0 { business } else { None });
            result.building_attributes.push(row[0x2c] as i8);
            result.entrances.push(std::array::from_fn(|axis| {
                f32::from_bits(word(row, 0x224 + axis * 4))
            }));
        }
        for (category, chain) in chains.iter().enumerate() {
            for &slot in chain {
                if businesses[slot].is_some() && word(&records[slot], 0x220) == 0 {
                    return Err("runtime business building has no entrance".into());
                }
                if result.building_categories[slot] != -1 {
                    return Err("duplicate runtime building membership".into());
                }
                result.building_categories[slot] = category as i16;
                result.chains[category].push(businesses[slot]);
                result.nodes[category].push(ShoppingNode {
                    building: slot as i32,
                    position: [
                        f32::from_bits(word(&records[slot], 0x0c)),
                        f32::from_bits(word(&records[slot], 0x10)),
                    ],
                });
            }
        }
        Ok(result)
    }

    pub fn category_owners(&self) -> [Option<usize>; 52] {
        self.categories
            .map(|category| category.owner.checked_sub(1).map(usize::from))
    }

    /// Rebuild commercial and residential entrances from original BuildFile
    /// geometry, then bind saved indices to those runtime destinations.
    pub fn reconstruct(
        save: &SaveFile<'_>,
        board: &Board,
        file: &crate::build_file::BuildFile<'_>,
    ) -> Result<Self, String> {
        let mut entrances = [[0.; 3]; 600];
        let geometry = board.category_geometry();
        for building in save.buildings()? {
            if i32::from_le_bytes(building.bytes[0x1c..0x20].try_into().unwrap()) == -1 {
                continue;
            }
            let scene = crate::scene::SceneBuilding::from_saved(&building, board, file)?;
            entrances[building.slot] = *scene
                .entrance_positions(geometry.centers[building.category])?
                .first()
                .ok_or("saved business building has no entrance")?;
        }
        Self::from_saved(save, board, &entrances)
    }

    /// Restore original category-list order and indices. Entrances are runtime
    /// geometry products, absent from saved prefixes, and must be supplied by
    /// the recovered placement path rather than inferred from resident positions.
    pub fn from_saved(
        save: &SaveFile<'_>,
        board: &Board,
        entrances: &[[f32; 3]; 600],
    ) -> Result<Self, String> {
        let mut businesses = Vec::with_capacity(600);
        let mut enabled = Vec::with_capacity(600);
        let mut attributes = Vec::with_capacity(600);
        for prefix in save.building_prefixes()?.chunks_exact(0x40) {
            let index = i32::from_le_bytes(prefix[0x1c..0x20].try_into().unwrap());
            let business = if index == -1 {
                None
            } else {
                let index = usize::try_from(index).map_err(|_| "negative saved business index")?;
                if index >= 600 {
                    return Err("saved business index exceeds pool".into());
                }
                Some(index)
            };
            businesses.push(business);
            enabled.push(if prefix[0x3f] != 0 { business } else { None });
            attributes.push(prefix[0x2c] as i8);
        }
        let mut result = Self {
            geometry: board.category_geometry(),
            crossing: board.crossing_topology()?,
            building_businesses: enabled,
            building_categories: vec![-1; 600],
            building_attributes: attributes,
            entrances: entrances.to_vec(),
            chains: std::array::from_fn(|_| Vec::new()),
            nodes: std::array::from_fn(|_| Vec::new()),
            category_masks: [0; 52],
            category_factors: [0.; 52],
            categories: crate::category::from_saved(save, &board.category_notification_values())?,
            category_counts: [0; 52],
        };
        for category in 0..52 {
            result.category_masks[category] = save.category_commodity_mask(category)?;
            result.category_factors[category] = save.category_prestige(category)?;
            result.category_counts[category] = save.category_resident_counter(category)?;
        }
        for building in save.buildings()? {
            if result.building_categories[building.slot] != -1 {
                return Err("duplicate saved building membership".into());
            }
            result.building_categories[building.slot] = building.category as i16;
            result.chains[building.category].push(businesses[building.slot]);
            result.nodes[building.category].push(ShoppingNode {
                building: building.slot as i32,
                position: building.center(),
            });
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original saved-world shopping choices"]
    fn matches_native_saved_choices() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let saved = read("OPENMT2001_SAVE_ONE");
        let save = crate::save::SaveFile::parse(&saved).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let build = crate::build_file::BuildFile::parse(&build).unwrap();
        let topology = super::Topology::reconstruct(&save, &board, &build).unwrap();
        let text = |name| String::from_utf8(read(name)).unwrap();
        let settings = super::Settings::from_saved(
            &save,
            &image,
            &text("OPENMT2001_DEFAULT_SETTINGS"),
            &text("OPENMT2001_BUSINESS_SETTINGS"),
            &text("OPENMT2001_PARAMETERS"),
        )
        .unwrap();
        let mut ledgers = crate::economy::Ledgers::from_saved(&save).unwrap();
        let initial_players = ledgers.players.clone();
        let world = crate::resident::PurchaseWorld {
            businesses: &ledgers.businesses,
            catalogs: &settings.catalogs.commodities,
            chains: &topology.chains,
            category_masks: &topology.category_masks,
            category_factors: &topology.category_factors,
            centers: &topology.geometry.centers,
            building_categories: &topology.building_categories,
            building_attributes: &topology.building_attributes,
            commodities: &settings.commodities,
            weights: &settings.weights,
            distance_disabled: false,
        };
        let bytes = read("OPENMT2001_NATIVE_SAVED_CHOICES");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSCHO01");
        assert_eq!(word(8) as usize, ledgers.residents.nodes.len());
        assert_eq!(word(12) as usize, ledgers.players.len());
        let mut at = 48;
        let mut successes = [0, 0];
        for (index, resident) in ledgers.residents.nodes.iter_mut().enumerate() {
            assert_eq!(word(at) as usize, index);
            let expected_returns = [word(at + 4) != 0, word(at + 8) != 0];
            at += 12;
            let mut auxiliary = [0; 284];
            resident
                .restore_saved_movement(0x10032000, &mut auxiliary, &topology.geometry.centers)
                .unwrap();
            assert_eq!(
                resident.as_bytes().as_slice(),
                &bytes[at..at + 330],
                "loaded resident{index}"
            );
            assert_eq!(
                auxiliary.as_slice(),
                &bytes[at + 330..at + 614],
                "loaded auxiliary{index}"
            );
            at += 614;
            ledgers.players.clone_from(&initial_players);
            let position =
                [4, 12].map(|o| f32::from_le_bytes(auxiliary[o..o + 4].try_into().unwrap()));
            let category = resident
                .choose_purchase_category(
                    &world,
                    &ledgers.residents.accounts,
                    position,
                    &topology.category_owners(),
                    &mut ledgers.players,
                )
                .unwrap();
            assert_eq!(category, expected_returns[0], "category result{index}");
            assert_eq!(
                resident.as_bytes().as_slice(),
                &bytes[at..at + 330],
                "category state{index}"
            );
            at += 330;
            let current = resident.as_bytes()[0x7c] as usize;
            let preferences: Vec<_> = settings
                .preferences
                .get(resident.as_bytes()[0x6a] as i8)
                .unwrap()
                .iter()
                .map(|p| p.commodity)
                .collect();
            let mask = if preferences.is_empty() {
                0
            } else {
                let account = &ledgers.residents.accounts[resident.account_index() as usize];
                u32::from_le_bytes(account.as_bytes()[0x84..0x88].try_into().unwrap())
            };
            let nodes: Vec<_> = topology.nodes[current]
                .iter()
                .zip(&topology.chains[current])
                .map(|(node, business)| crate::resident::PurchaseNode {
                    building: node.building,
                    position: node.position,
                    business_mask: business.map(|b| {
                        u32::from_le_bytes(ledgers.businesses[b][0x70..0x74].try_into().unwrap())
                    }),
                })
                .collect();
            let business = resident
                .choose_purchase_business(
                    &world,
                    &ledgers.residents.accounts,
                    crate::resident::PurchaseBusinessInputs {
                        preferences: &preferences,
                        account_mask: mask,
                        nodes: &nodes,
                        auxiliary_position: position,
                        distance_weight: settings.business_distance_weights
                            [resident.class() as usize],
                    },
                    &topology.entrances,
                )
                .unwrap();
            assert_eq!(business, expected_returns[1], "business result{index}");
            assert_eq!(
                resident.as_bytes().as_slice(),
                &bytes[at..at + 330],
                "business state{index}"
            );
            at += 330;
            for player in &ledgers.players {
                let expected =
                    crate::economy::PlayerAccount::from_saved(&crate::save::SavedPlayer {
                        metadata: &[0; 48],
                        bytes: bytes[at..at + 440].try_into().unwrap(),
                    })
                    .unwrap();
                assert_eq!(
                    player.retail_counters, expected.retail_counters,
                    "owner counters{index}"
                );
                assert_eq!(player.cash.to_bits(), expected.cash.to_bits());
                at += 440;
            }
            successes[0] += usize::from(category);
            successes[1] += usize::from(business);
        }
        assert_eq!(successes, [5, 2]);
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original settings, saves and native settings observations"]
    fn matches_native_saved_settings_and_catalogs() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let source = String::from_utf8(read("OPENMT2001_BUSINESS_SETTINGS")).unwrap();
        let defaults = String::from_utf8(read("OPENMT2001_DEFAULT_SETTINGS")).unwrap();
        let parameters = String::from_utf8(read("OPENMT2001_PARAMETERS")).unwrap();
        let catalogs = super::Catalogs::from_v14_defaults(&source, &image).unwrap();
        let bytes = read("OPENMT2001_NATIVE_CATALOG");
        assert_eq!(&bytes[..8], b"MTCAT001");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 8;
        for kind in 0..45 {
            let count = word(at) as usize;
            let mask = word(at + 4);
            at += 8;
            assert_eq!(catalogs.commodities[kind].len(), count);
            assert_eq!(
                catalogs.commodities[kind]
                    .iter()
                    .fold(0u32, |m, c| m | (1 << c)),
                mask
            );
            for slot in 0..count {
                assert_eq!(catalogs.commodities[kind][slot], word(at));
                assert_eq!(catalogs.fractions[kind][slot].to_bits(), word(at + 4));
                at += 8;
            }
        }
        assert_eq!(at, bytes.len());
        let bytes = read("OPENMT2001_NATIVE_SAVED_SETTINGS");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSBND01");
        assert_eq!((word(8), word(16), word(20)), (1, 2, 0x027f));
        let mut at = 56;
        for _ in 0..word(12) {
            let name_len = word(at) as usize;
            let source_len = word(at + 4) as usize;
            at += 12 + name_len;
            let source = std::str::from_utf8(&bytes[at..at + source_len]).unwrap();
            at += source_len;
            let weights = crate::resident::ClassWeights::from_parameters(source, &image).unwrap();
            for value in weights.rows.into_iter().flatten() {
                assert_eq!(value.to_bits(), word(at));
                at += 4;
            }
        }
        for name in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let data = read(name);
            let save = crate::save::SaveFile::parse(&data).unwrap();
            let settings =
                super::Settings::from_saved(&save, &image, &defaults, &source, &parameters)
                    .unwrap();
            let info_len = word(at + 32) as usize;
            let commodity_len = word(at + 36) as usize;
            at += 40 + info_len + commodity_len;
            for value in settings
                .interaction_delays
                .into_iter()
                .chain(settings.unit_costs)
            {
                assert_eq!(value.to_bits(), word(at));
                at += 4;
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires supplied retail geometry and original entrance observations"]
    fn matches_native_saved_retail_entrances() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let board =
            crate::board::Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let data = read("OPENMT2001_SAVE_ONE");
        let save = crate::save::SaveFile::parse(&data).unwrap();
        let build = read("OPENMT2001_BUILD_FILE");
        let file = crate::build_file::BuildFile::parse(&build).unwrap();
        let topology = super::Topology::reconstruct(&save, &board, &file).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SAVED_RETAIL_ENTRANCES");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRETL01");
        assert_eq!((word(8), word(12), word(16)), (1, 6, 0x027f));
        let mut at = 84;
        for _ in 0..6 {
            let slot = word(at) as usize;
            let count = word(at + 36) as usize;
            at += 44;
            let saved = save
                .buildings()
                .unwrap()
                .into_iter()
                .find(|b| b.slot == slot)
                .unwrap();
            let scene = crate::scene::SceneBuilding::from_saved(&saved, &board, &file).unwrap();
            let points = scene
                .entrance_positions(topology.geometry.centers[saved.category])
                .unwrap();
            assert_eq!(points.len(), count);
            at += count * 12; // Native emitted points before world/category placement.
            for point in &points {
                assert_eq!(
                    point.map(f32::to_bits),
                    [word(at), word(at + 4), word(at + 8)],
                    "building{slot}"
                );
                at += 12;
            }
            assert_eq!(topology.entrances[slot], points[0]);
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires supplied saves and original saved shopping bindings"]
    fn matches_native_saved_bindings() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let board =
            crate::board::Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SAVED_SHOPPING");
        assert_eq!(&bytes[..8], b"MTSBIND1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 2);
        let mut at = 12;
        for name in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let data = read(name);
            let save = crate::save::SaveFile::parse(&data).unwrap();
            let topology = super::Topology::from_saved(&save, &board, &[[0.; 3]; 600]).unwrap();
            at += 32; // Original input hash retained by the native generator.
            for category in 0..52 {
                assert_eq!(topology.category_masks[category], word(at));
                assert_eq!(topology.category_factors[category].to_bits(), word(at + 4));
                assert_eq!(
                    topology.category_owners()[category],
                    word(at + 8).checked_sub(1).map(|i| i as usize)
                );
                assert_eq!(topology.category_counts[category], word(at + 12));
                assert_eq!(topology.nodes[category].len(), word(at + 16) as usize);
                at += 20;
                for node in &topology.nodes[category] {
                    assert_eq!(node.building, word(at) as i32);
                    at += 4;
                }
            }
            let records = at;
            for slot in 0..600 {
                let business = word(at) as i32;
                let mapped = (business != -1).then_some(business as usize);
                assert_eq!(
                    topology.building_businesses[slot],
                    if word(at + 8) != 0 { mapped } else { None }
                );
                assert_eq!(
                    i32::from(topology.building_categories[slot]),
                    word(at + 4) as i32
                );
                assert_eq!(
                    i32::from(topology.building_attributes[slot]),
                    word(at + 12) as i32
                );
                at += 24;
            }
            for category in 0..52 {
                for (node, business) in topology.nodes[category]
                    .iter()
                    .zip(&topology.chains[category])
                {
                    let row = records + node.building as usize * 24;
                    let index = word(row) as i32;
                    assert_eq!(*business, (index != -1).then_some(index as usize));
                    assert_eq!(
                        node.position.map(f32::to_bits),
                        [word(row + 16), word(row + 20)]
                    );
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
}
