//! Retained normal-city population state over the authoritative fresh world.
//! Host timing, script dispatch and renderer submission are separate boundaries.
use crate::{level_startup::City, shopping};

impl crate::hourly::World for City {
    fn players(&self) -> &[crate::economy::PlayerAccount] {
        &self.ledgers.players
    }
}

pub struct HourlyContext<'a> {
    pub board: &'a crate::board::Board,
    pub calendar: crate::clock::Controls,
    pub hourly: crate::hourly::Controls,
    pub utilities: &'a mut crate::economy::Utilities,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CalendarEvent {
    Clock(crate::clock::Event),
    Hourly(crate::hourly::Event),
    AccountPosted(crate::economy::AccountPosting),
}

pub struct Population {
    pub shopping: shopping::Topology,
    pub housing: crate::housing::Topology,
    pub settings: shopping::Settings,
    pub housing_settings: crate::housing::Settings,
    pub routes: crate::routing::CategoryRoutes,
    pub bindings: Vec<shopping::ResidentBinding>,
    pub animations: crate::resident::ResidentAnimationPool,
    pub animation_settings: crate::resident::ResidentAnimationSettings,
    pub properties: crate::economy::PropertyIncome,
    pub market: crate::market::Cache,
    pub business_counts: crate::player_metrics::BusinessCounts,
}
impl Population {
    pub fn sample_history(
        &mut self,
        history: &mut crate::player_history::State,
        city: &City,
        players: &[[u8; 440]],
        image: &crate::pe::PeImage<'_>,
    ) -> Result<(), String> {
        history.sample(players, |metric, slot| {
            use crate::player_history::{Metric, Value};
            Ok(match metric {
                Metric::EmpireValue => Value::Float(
                    crate::player_metrics::runtime_metric(
                        city,
                        &players[slot],
                        slot,
                        0x47fbb0,
                        image,
                        0x591318,
                    )?
                    .to_f32(),
                ),
                Metric::CurrentLedgerIncome => {
                    Value::Float(crate::player_metrics::current_income(&players[slot])?.to_f32())
                }
                Metric::BusinessCount => Value::Integer(self.business_counts.get(
                    slot,
                    players.len(),
                    &city.ledgers.businesses,
                )?),
            })
        })
    }
    pub fn drain_business_commands(
        &mut self,
        commands: &mut crate::economy::LocalBusinessCommands,
        clock: &mut crate::clock::Clock,
        city: &mut City,
        board: &crate::board::Board,
    ) -> Result<(), String> {
        commands.drain((&mut clock.timing[..2]).try_into().unwrap(), |command| {
            self.apply_business_terms(city, board, command)
        })
    }

    /// Hourly lease expiry publishes its mutable projections on both success
    /// and failure, before another script or finance operation can observe it.
    pub fn expire_categories(
        &mut self,
        city: &mut City,
        board: &crate::board::Board,
        context: &mut crate::hourly::CategoryContext<'_>,
        callout: impl FnMut(
            crate::category::Call,
            &mut [crate::category::State; 52],
            &mut crate::category::Controls,
            &mut crate::category::Expiry<'_>,
        ) -> Result<(), String>,
        event: impl FnMut(
            crate::category::ExpiryEvent,
            &mut crate::category::Expiry<'_>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.refresh_city(city, board)?;
        let mut buildings = building_prefixes(city);
        let chains = std::array::from_fn(|category| {
            self.shopping.nodes[category]
                .iter()
                .map(|node| node.building as usize)
                .collect()
        });
        let mut expiry = crate::category::Expiry {
            buildings: &mut buildings,
            render_kinds: context.render_kinds,
            chains: &chains,
            ledgers: &mut city.ledgers,
            normal_tick: context.normal_tick,
            accounting_enabled: context.accounting_enabled,
            refresh_counter: city.ownership_refresh_counter,
        };
        let result = expiry.advance_hour(
            &mut self.shopping.categories,
            &mut context.controls,
            callout,
            event,
        );
        context.normal_tick = expiry.normal_tick;
        context.accounting_enabled = expiry.accounting_enabled;
        context.refresh_counter = expiry.refresh_counter;
        city.ownership_refresh_counter = expiry.refresh_counter;
        self.shopping.write_category_ownership(&mut city.categories);
        for (row, prefix) in city.buildings.iter_mut().zip(buildings) {
            row[..64].copy_from_slice(&prefix);
        }
        result
    }

    /// Complete clock/hourly ordering over one city, with retained population
    /// state. Every unbound script, objective, controller or host operation is
    /// exposed synchronously and may change the next operation's inputs.
    pub fn advance_hour(
        &mut self,
        clock: &mut crate::clock::Clock,
        city: &mut City,
        context: &mut HourlyContext<'_>,
        mut boundary: impl FnMut(
            CalendarEvent,
            &mut crate::clock::Clock,
            &mut City,
            &mut Self,
            &mut HourlyContext<'_>,
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        // The clock captures years_per_day before Hourly, just as native ESI.
        // Controls are small live registers, copied back at every boundary so
        // callbacks can change later gates. City records never move or clone.
        context.calendar.years_per_day = city.settings.years_per_day();
        let mut calendar = context.calendar;
        let result = clock.try_advance_hour(&mut calendar, |event, clock, calendar| {
            context.calendar = *calendar;
            let result = if event == crate::clock::Event::Hourly {
                // Hourly controls must likewise survive failed callbacks.
                let mut hourly = crate::hourly::Controls {
                    suppressed: context.hourly.suppressed,
                    world_mode: context.hourly.world_mode,
                    before_six_sound: context.hourly.before_six_sound,
                    network_enabled: context.hourly.network_enabled,
                    network_override: context.hourly.network_override,
                };
                let result = crate::hourly::advance_world(
                    clock,
                    &mut hourly,
                    city,
                    |event, clock, hourly, city| {
                        std::mem::swap(hourly, &mut context.hourly);
                        let mut postings = Vec::new();
                        let result = self
                            .hourly_event(
                                event,
                                city,
                                clock,
                                context.board,
                                context.utilities,
                                &mut postings,
                            )
                            .and_then(|handled| {
                                for posting in postings {
                                    boundary(
                                        CalendarEvent::AccountPosted(posting),
                                        clock,
                                        city,
                                        self,
                                        context,
                                    )?;
                                }
                                if handled {
                                    Ok(false)
                                } else {
                                    boundary(
                                        CalendarEvent::Hourly(event),
                                        clock,
                                        city,
                                        self,
                                        context,
                                    )
                                }
                            });
                        std::mem::swap(hourly, &mut context.hourly);
                        result
                    },
                );
                context.hourly = hourly;
                result.map(|()| false)
            } else {
                boundary(CalendarEvent::Clock(event), clock, city, self, context)
            };
            *calendar = context.calendar;
            result
        });
        context.calendar = calendar;
        result
    }

    /// Bind an hourly event to the current city. Derived read-only views are
    /// rebuilt at the consuming boundary, after preceding Lua/expiry effects.
    /// False means the event still requires a synchronous world/host handler.
    pub fn hourly_event(
        &mut self,
        event: crate::hourly::Event,
        city: &mut City,
        clock: &crate::clock::Clock,
        board: &crate::board::Board,
        utilities: &mut crate::economy::Utilities,
        postings: &mut Vec<crate::economy::AccountPosting>,
    ) -> Result<bool, String> {
        use crate::hourly::Event;
        match event {
            Event::Businesses => city.ledgers.update_hourly_businesses(clock.hour()),
            Event::CloseBusinessDay => {
                self.refresh_city(city, board)?;
                let counts =
                    std::array::from_fn(|i| self.settings.catalogs.commodities[i].len() as u8);
                city.ledgers.close_business_day(&counts)?;
            }
            Event::RotatePlayerLedgers => {
                for player in &mut city.ledgers.players {
                    player.rotate_ledger();
                }
            }
            Event::FinishPlayerDay => {
                for player in &mut city.ledgers.players {
                    player.finish_daily_update();
                }
            }
            Event::DailyBusinesses => {
                self.refresh_city(city, board)?;
                let buildings = building_prefixes(city);
                let owners = self.shopping.categories.map(|category| category.owner);
                utilities.owners = [2, 35, 39, 42].map(|i| owners[i]);
                let business =
                    crate::economy::BusinessDaySettings::from_catalog(&city.catalog.info);
                let parameters = crate::economy::BusinessDayInputs {
                    buildings: &buildings,
                    building_categories: &self.shopping.building_categories,
                    category_owners: &owners,
                    catalogs: &self.settings.catalogs,
                    unit_prices: &self.settings.unit_costs,
                    settings: &business,
                }
                .parameters(&city.ledgers.businesses)?;
                postings.extend(crate::economy::update_daily_businesses(
                    &mut city.ledgers.businesses,
                    &parameters,
                    &mut city.ledgers.residents,
                    &mut city.ledgers.players,
                    utilities,
                )?);
            }
            Event::Helper(0x48e070) => {
                // Ownership can change in an earlier callback in this hour.
                self.refresh_city(city, board)?;
                let owners = self.shopping.categories.map(|category| category.owner);
                utilities.owners = [2, 35, 39, 42].map(|i| owners[i]);
                self.properties.owners = [4, 17, 43, 44].map(|i| owners[i]);
                postings.extend(city.settings.selected_income().pay_owners(
                    utilities,
                    &mut self.properties,
                    &mut city.ledgers.players,
                )?);
            }
            Event::MorningResidents => {
                self.refresh_city(city, board)?;
                city.ledgers.residents.update_daily(
                    &city.welfare,
                    &self.settings.preferences,
                    &self.settings.commodities,
                    &city.configuration.objectives,
                    &mut city.creation.initialization_rng.objective,
                )?;
            }
            Event::EveningResidents => {
                self.refresh_city(city, board)?;
                let buildings = building_prefixes(city);
                let mut average_rent = |businesses: &[[u8; 544]]| {
                    self.market.residential_rent(&crate::market::Inputs {
                        businesses,
                        buildings: &buildings,
                        catalogs: &self.settings.catalogs,
                        commodities: &self.settings.commodities,
                    })
                };
                let mut housing = crate::housing::World {
                    demand: Some(&mut city.ai_demand),
                    buildings: &self.housing.buildings,
                    categories: &self.housing.categories,
                    category_values: &self.shopping.category_factors,
                    businesses: &mut city.ledgers.businesses,
                    available: &mut city.available,
                    players: &mut city.ledgers.players,
                    welfare: &city.welfare,
                    settings: &self.housing_settings,
                    average_rent: &mut average_rent,
                };
                let mut countdown = crate::housing::Countdown {
                    state: city.creation.initialization_rng.constructor,
                    multiplier: 0x343fd,
                    increment: 0x269ec3,
                    period: city.settings.itchy_counter,
                };
                let result = crate::housing::update_evening(
                    &mut city.ledgers.residents,
                    &mut housing,
                    &city.configuration.objectives,
                    &mut city.creation.initialization_rng.objective,
                    &mut countdown,
                );
                city.creation.initialization_rng.constructor = countdown.state;
                result?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Apply command18 to its stored business identity, including immediate
    /// tenant reevaluation after rent increases. The queue owns admission and
    /// timing; this method owns only the original command's city effects.
    pub fn apply_business_terms(
        &mut self,
        city: &mut City,
        board: &crate::board::Board,
        command: &crate::economy::PendingBusinessTerms,
    ) -> Result<(), String> {
        self.refresh_city(city, board)?;
        let buildings = building_prefixes(city);
        let mut average_rent = |businesses: &[[u8; 544]]| {
            self.market.residential_rent(&crate::market::Inputs {
                businesses,
                buildings: &buildings,
                catalogs: &self.settings.catalogs,
                commodities: &self.settings.commodities,
            })
        };
        let mut housing = crate::housing::World {
            demand: Some(&mut city.ai_demand),
            buildings: &self.housing.buildings,
            categories: &self.housing.categories,
            category_values: &self.shopping.category_factors,
            businesses: &mut city.ledgers.businesses,
            available: &mut city.available,
            players: &mut city.ledgers.players,
            welfare: &city.welfare,
            settings: &self.housing_settings,
            average_rent: &mut average_rent,
        };
        crate::housing::apply_business_terms(
            &mut city.ledgers.residents,
            &mut housing,
            usize::try_from(command.business_index)
                .map_err(|_| "negative business command identity")?,
            &command.terms,
            &city.configuration.objectives,
            &mut city.creation.initialization_rng.objective,
        )
    }

    pub fn new(
        world: &City,
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        parameters: &str,
        models: &[crate::economy::ResidentModelBinding; 32],
        people: &crate::people::PeopleModels<'_>,
    ) -> Result<Self, String> {
        let mut state = Self {
            shopping: shopping::Topology::from_runtime(
                &world.buildings,
                &world.categories,
                0x591318,
                board,
            )?,
            housing: crate::housing::Topology::from_runtime(
                &world.buildings,
                &world.categories,
                0x591318,
            )?,
            settings: shopping::Settings::from_runtime(
                &world.catalog,
                &world.commodities,
                &world.configuration,
            )?,
            housing_settings: crate::housing::Settings::from_parameters(parameters, image)?,
            routes: world.category_routes(board),
            bindings: Vec::new(),
            animations: crate::resident::ResidentAnimationPool::default(),
            animation_settings: crate::resident::ResidentAnimationSettings::from_image(image)?,
            properties: crate::economy::PropertyIncome {
                owners: [0; 4],
                totals: [0.; 4],
            },
            market: crate::market::Cache {
                dirty: true,
                prices: [0.; 32],
                rent: 0.,
            },
            business_counts: crate::player_metrics::BusinessCounts {
                age: 100,
                values: [0; 6],
            },
        };
        state.refresh_bindings(world, models, people)?;
        Ok(state)
    }
    /// Rebuild derived topology after script/construction/ownership changes.
    /// Tick-owned category counts are published before returning from a pass.
    pub fn refresh_city(
        &mut self,
        world: &City,
        board: &crate::board::Board,
    ) -> Result<(), String> {
        self.shopping =
            shopping::Topology::from_runtime(&world.buildings, &world.categories, 0x591318, board)?;
        self.housing =
            crate::housing::Topology::from_runtime(&world.buildings, &world.categories, 0x591318)?;
        self.settings = shopping::Settings::from_runtime(
            &world.catalog,
            &world.commodities,
            &world.configuration,
        )?;
        Ok(())
    }
    pub fn refresh_bindings(
        &mut self,
        world: &City,
        models: &[crate::economy::ResidentModelBinding; 32],
        people: &crate::people::PeopleModels<'_>,
    ) -> Result<(), String> {
        let count = world.ledgers.residents.active_count as usize;
        let residents = world
            .ledgers
            .residents
            .nodes
            .get(..count)
            .ok_or("active resident count leaves pool")?;
        self.bindings = residents
            .iter()
            .enumerate()
            .map(|(index, resident)| {
                let reference = resident.auxiliary_reference();
                // Fresh resident auxiliaries are allocated in resident order; keep
                // the original opaque reference for animation identity checks.
                let aux = world
                    .creation
                    .auxiliary
                    .slots()
                    .get(index)
                    .ok_or("resident auxiliary missing")?;
                let model = u32::from_le_bytes(aux[..4].try_into().unwrap());
                let row = models
                    .iter()
                    .position(|binding| binding.reference == model)
                    .ok_or("resident model reference missing")?;
                Ok(shopping::ResidentBinding {
                    resident_reference: 0x6fd7e8 + index as u32 * 0x150,
                    auxiliary_reference: reference,
                    auxiliary_slot: index,
                    model_frame_count: u16::try_from(
                        people
                            .geometry_source(row, false)
                            .ok_or("resident model geometry missing")?
                            .frames,
                    )
                    .map_err(|_| "resident frame count overflow")?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(())
    }
    /// People.Create appends residents and auxiliaries while running a script.
    /// The resident dispatcher must see those new handles before its next pass.
    pub fn refresh_bindings_if_changed(
        &mut self,
        world: &City,
        models: &[crate::economy::ResidentModelBinding; 32],
        people: &crate::people::PeopleModels<'_>,
    ) -> Result<(), String> {
        if self.bindings.len() != world.ledgers.residents.active_count as usize {
            self.refresh_bindings(world, models, people)?;
        }
        Ok(())
    }
    pub fn tick_residents(
        &mut self,
        world: &mut City,
        tick: u32,
        hour: u32,
        distance_disabled: bool,
        postings: &mut Vec<crate::economy::AccountPosting>,
        other: impl FnMut(
            usize,
            crate::resident::ActionHelper,
            &mut crate::resident::Resident,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        let enabled = world.categories.map(|row| row[4]);
        let mut rng = [
            world.creation.initialization_rng.constructor,
            0x343fd,
            0x269ec3,
        ];
        let mut population = shopping::Population {
            postings,
            topology: &mut self.shopping,
            settings: &self.settings,
            routes: &mut self.routes,
            lanes: &mut world.road_network.lanes,
            auxiliary: &mut world.creation.auxiliary,
            animations: &mut self.animations,
            animation_settings: &self.animation_settings,
            bindings: &self.bindings,
        };
        let result = population.tick_with_city_actions(
            &mut world.ledgers,
            shopping::PopulationTick {
                tick,
                boundary: world.configuration.movement_boundary,
                distance_disabled,
            },
            &mut shopping::CityActions {
                demand: Some(&mut world.ai_demand),
                housing: &self.housing,
                housing_settings: &self.housing_settings,
                welfare: &world.welfare,
                objectives: &world.configuration.objectives,
                objective_rng: &mut world.creation.initialization_rng.objective,
                constructor_rng: &mut rng,
                available: &mut world.available,
                category_enabled: &enabled,
                properties: &mut self.properties,
            },
            |_, event, resident, _| {
                if event != crate::resident::TickEvent::RefreshIntent {
                    return Err(format!("unexpected population tick boundary {event:?}"));
                }
                resident.refresh_intent(
                    &world.configuration.timetables,
                    hour as usize,
                    world.commodities.memberships,
                )?;
                Ok(false)
            },
            other,
        );
        world.creation.initialization_rng.constructor = rng[0];
        self.shopping.write_resident_counts(&mut world.categories);
        result
    }
    pub fn advance_animations(
        &mut self,
        world: &mut City,
        other: impl FnMut(
            usize,
            &mut crate::resident::ResidentAnimationPool,
            &mut crate::resident::AuxiliaryPool,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        shopping::Population {
            postings: &mut Vec::new(),
            topology: &mut self.shopping,
            settings: &self.settings,
            routes: &mut self.routes,
            lanes: &mut world.road_network.lanes,
            auxiliary: &mut world.creation.auxiliary,
            animations: &mut self.animations,
            animation_settings: &self.animation_settings,
            bindings: &self.bindings,
        }
        .advance_animations(&mut world.ledgers.residents, other)
    }
}

fn building_prefixes(city: &City) -> Vec<[u8; 64]> {
    city.buildings
        .iter()
        .map(|row| row[..64].try_into().unwrap())
        .collect()
}

/// Notifications from the retained dispatcher, including completed account posts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event<T> {
    Scheduled(T),
    AccountPosted(crate::economy::AccountPosting),
}

/// The normal scheduler borrows retained population and traffic state. The
/// callback must implement every other world/host boundary synchronously;
/// returning success is never supplied implicitly for an unknown operation.
pub struct Dispatch<'a, M> {
    pub population: &'a mut Population,
    pub roads: &'a mut crate::traffic::road::runtime::RoadTraffic,
    pub routes: &'a crate::traffic::road::routes::RoadRoutes<'a>,
    pub models: &'a mut M,
    pub camera: [f32; 3],
    pub distance_disabled: bool,
}
impl<M: crate::traffic::road::runtime::VehicleModels> Dispatch<'_, M> {
    pub fn advance(
        &mut self,
        world: &mut crate::level_startup::World,
        controls: &mut crate::scheduler::Controls,
        boundary: impl FnMut(
            Event<crate::scheduler::Event>,
            &mut crate::clock::Clock,
            &mut City,
            &mut crate::scheduler::Controls,
            &mut Self,
        ) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.advance_with_animations(world, controls, || Ok(()), boundary)
    }

    /// Host presentation sequences share481819's dispatcher with residents.
    /// Their callback inherits native pause/mode/iteration gates and runs once
    /// per actual animation dispatch, independently of host frame frequency.
    pub fn advance_with_animations(
        &mut self,
        world: &mut crate::level_startup::World,
        controls: &mut crate::scheduler::Controls,
        mut animations: impl FnMut() -> Result<(), String>,
        mut boundary: impl FnMut(
            Event<crate::scheduler::Event>,
            &mut crate::clock::Clock,
            &mut City,
            &mut crate::scheduler::Controls,
            &mut Self,
        ) -> Result<u32, String>,
    ) -> Result<(), String> {
        crate::scheduler::advance(&mut world.clock, controls, |event, clock, controls| {
            let mut postings = Vec::new();
            let result = self.event(event, clock, &mut world.city, &mut postings);
            // A later resident failure does not undo preceding account posts.
            for posting in postings {
                boundary(
                    Event::AccountPosted(posting),
                    clock,
                    &mut world.city,
                    controls,
                    self,
                )?;
            }
            if result? {
                if matches!(event, crate::scheduler::Event::Animations) {
                    animations()?;
                }
                Ok(0)
            } else {
                boundary(
                    Event::Scheduled(event),
                    clock,
                    &mut world.city,
                    controls,
                    self,
                )
            }
        })
    }

    /// Dispatch elapsed host time against the same live timer registrations.
    /// Script callbacks can synchronously edit or append those registrations.
    pub fn advance_timed(
        &mut self,
        world: &mut crate::level_startup::World,
        controls: &mut crate::scheduler::Controls,
        frame: &mut crate::timers::Frame,
        tasks: &mut Vec<crate::timers::Task>,
        milliseconds: Option<u32>,
        mut boundary: impl FnMut(
            Event<crate::scheduler::TimedEvent>,
            &mut crate::clock::Clock,
            &mut City,
            &mut crate::scheduler::Controls,
            &mut Vec<crate::timers::Task>,
            &mut Self,
        ) -> Result<u32, String>,
    ) -> Result<Vec<f32>, String> {
        crate::scheduler::advance_timed(
            milliseconds,
            frame,
            tasks,
            &mut world.clock,
            controls,
            |event, clock, controls, tasks| {
                if let crate::scheduler::TimedEvent::Logic(logic) = event {
                    let mut postings = Vec::new();
                    let result = self.event(logic, clock, &mut world.city, &mut postings);
                    for posting in postings {
                        boundary(
                            Event::AccountPosted(posting),
                            clock,
                            &mut world.city,
                            controls,
                            tasks,
                            self,
                        )?;
                    }
                    if result? {
                        return Ok(0);
                    }
                }
                boundary(
                    Event::Scheduled(event),
                    clock,
                    &mut world.city,
                    controls,
                    tasks,
                    self,
                )
            },
        )
    }

    fn event(
        &mut self,
        event: crate::scheduler::Event,
        clock: &mut crate::clock::Clock,
        city: &mut City,
        postings: &mut Vec<crate::economy::AccountPosting>,
    ) -> Result<bool, String> {
        match event {
            crate::scheduler::Event::Residents => self.population.tick_residents(
                city,
                clock.timing[0],
                clock.hour(),
                self.distance_disabled,
                postings,
                |index, helper, _, _| Err(format!("resident {index} requires {helper:?}")),
            )?,
            crate::scheduler::Event::Animations => {
                self.population.advance_animations(city, |index, _, _| {
                    Err(format!("nonresident animation {index} requires dispatcher"))
                })?
            }
            crate::scheduler::Event::Helper(0x44c210) => self.roads.advance(
                &mut city.road_network,
                self.routes,
                crate::traffic::road::runtime::Tick {
                    hour: clock.hour() as i32,
                    era: clock.calendar[2],
                    frequencies: &city.settings.traffic,
                    camera: self.camera,
                },
                self.models,
            )?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}
