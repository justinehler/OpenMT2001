//! Shared fresh-city storage and original normal-level script startup.
//! Filesystem/GPU work, process RNG reseeding and roster allocation are host
//! boundaries. This is not the complete post-roster world initialization.
use crate::{level_script, lua4::Runtime};

/// Host-owned identity/registration inputs and the live801dec event gate.
/// Auxiliary prior bytes are explicit because the original stack template
/// leaves some fields untouched; a zero-filled host policy is not native data.
pub struct HostContext {
    /// Incoming control for this invocation; early CRT callers pass PC53.
    pub precision: crate::x87::Precision,
    pub difficulty: i32,
    pub notifications: bool,
    pub accounting_enabled: bool,
    pub auxiliary_template: [u8; 0x11c],
    pub category_base: u32,
    pub building_base: u32,
}

pub struct World {
    pub clock: crate::clock::Clock,
    pub city: City,
}
/// Mutable city records separate from the clock so synchronous scheduler
/// callbacks can borrow both without a copied or temporarily detached clock.
/// Successful version1.4 topology initialization result. Native449a40 writes
/// count52 after scanning property geometry, including inactive rows. It is
/// not the number of visible Board blocks, loaded meshes or enabled rows.
/// No public/default constructor or independent host setter exists.
/// Process-local World identity, minted only by actual topology initialization.
/// Not a native address, serialized id, geometry id, filename or timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceEpoch(u64);
static NEXT_SOURCE_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn mint_source_epoch(next: &std::sync::atomic::AtomicU64) -> Result<SourceEpoch, String> {
    next.fetch_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |value| value.checked_add(1),
    )
    .map(SourceEpoch)
    .map_err(|_| "World source epoch exhausted".into())
}
impl SourceEpoch {
    pub fn serial(self) -> u64 {
        self.0
    }
}
/// World identity a property source was captured from. Production values come
/// only from World::fresh's checked topology writer; `Synthetic` identities are
/// test fixtures (behind `test-support` so the binary's tests can build them)
/// and never follow or precede a World epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropertyEpoch {
    World(SourceEpoch),
    #[cfg(any(test, feature = "test-support"))]
    Synthetic(u64),
}
impl PropertyEpoch {
    pub fn serial(self) -> u64 {
        match self {
            Self::World(epoch) => epoch.serial(),
            #[cfg(any(test, feature = "test-support"))]
            Self::Synthetic(value) => value,
        }
    }
    pub fn follows(self, old: Self) -> bool {
        match (self, old) {
            (Self::World(a), Self::World(b)) => a > b,
            #[cfg(any(test, feature = "test-support"))]
            (Self::Synthetic(a), Self::Synthetic(b)) => a > b,
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }
    /// Test support: a synthetic identity.
    #[cfg(any(test, feature = "test-support"))]
    pub fn synthetic(value: u64) -> Self {
        Self::Synthetic(value)
    }
}
#[derive(Debug)]
pub struct Topology {
    count617624: i32,
    epoch: SourceEpoch,
}
impl Topology {
    fn after_v14_geometry() -> Result<Self, String> {
        Ok(Self {
            count617624: 52,
            epoch: mint_source_epoch(&NEXT_SOURCE_EPOCH)?,
        })
    }
    pub fn epoch(&self) -> SourceEpoch {
        self.epoch
    }
    pub fn count617624(&self) -> i32 {
        self.count617624
    }
}
pub struct City {
    /// Readonly projection for this Session's admitted parameter source.
    /// Rebuilt on cold startup; materialized saved building rates stay owned.
    pub building_parameters: crate::building_parameters::BuildingKinds,
    pub topology: Topology,
    pub objective: crate::scenario_objective::State,
    pub settings: crate::startup_settings::State,
    pub configuration: crate::resident::Configuration,
    pub welfare: crate::resident::Welfare,
    pub pause: crate::pause::State,
    pub buildings: Vec<[u8; 0x394]>,
    pub ledgers: crate::economy::Ledgers,
    pub categories: [[u8; 0x17c]; 52],
    pub category_footprints: [[i32; 2]; 52],
    pub ai_neighbors: crate::ai_neighbors::Cache,
    pub ai_demand: crate::ai_demand::State,
    pub road_network: crate::traffic::road::construction::RoadNetwork,
    pub definitions: crate::initial_conditions::Definitions,
    pub commodities: crate::commodity_script::State,
    pub catalog: crate::business_script::State,
    pub creation: crate::economy::ResidentCreationState,
    pub available: Vec<usize>,
    pub availability_cursor: Option<usize>,
    pub player_money: f32,
    pub construction_components: [f32; 4],
    pub pending_world: crate::pending_world::State,
    pub ownership_refresh_counter: u8,
    pub auction: crate::ai_messages::offers::auction::State,
}
impl City {
    /// Original00449150/0045ebd0 read006fcd3c, registered as
    /// People.subwaythreshold by004dd1ad. Build after DEFAULT/scenario scripts.
    pub fn category_routes(&self, board: &crate::board::Board) -> crate::routing::CategoryRoutes {
        crate::routing::CategoryRoutes::from_board(board, self.settings.subway_threshold)
    }
}
impl std::ops::Deref for World {
    type Target = City;
    fn deref(&self) -> &City {
        &self.city
    }
}
impl std::ops::DerefMut for World {
    fn deref_mut(&mut self) -> &mut City {
        &mut self.city
    }
}
impl World {
    /// Fresh city for a clean process; same-process starts use fresh_with_objective.
    /// The caller supplies creation RNG/auxiliary state; no new seed is invented.
    pub fn fresh(
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        configuration: crate::resident::Configuration,
        creation: crate::economy::ResidentCreationState,
    ) -> Result<Self, String> {
        Self::fresh_with_objective(image, board, configuration, creation, Default::default())
    }

    /// The process-owned controller survives city replacement. Reset before
    /// any new level scripts, retaining excess words and padding (0x489490).
    pub fn fresh_with_objective(
        image: &crate::pe::PeImage<'_>,
        board: &crate::board::Board,
        configuration: crate::resident::Configuration,
        creation: crate::economy::ResidentCreationState,
        mut objective: crate::scenario_objective::State,
    ) -> Result<Self, String> {
        objective.reset();
        let mut clock = crate::clock::Clock {
            calendar: [0; 5],
            timing: [0, 0, 0, 0, 0, 1],
        };
        clock.reset_world_clock();
        let mut residents = crate::economy::ResidentAccounts::new_pool();
        residents.reset_account_pool()?;
        let definitions = crate::initial_conditions::Definitions::from_v14(image)?;
        let mut buildings = vec![[0; 0x394]; 600];
        crate::building_pool::reset(&mut buildings, &mut [0; 0x394])?;
        let mut businesses = vec![[0; 0x220]; 600];
        crate::business_pool::BusinessPool::new(businesses.as_mut_slice().try_into().unwrap())
            .reset_records();
        let mut categories = [[0; 0x17c]; 52];
        crate::category_reset::reset_after_lists_removed(
            &mut categories,
            &definitions.reset_values(),
        );
        crate::ai_neighbors::bind_geometry(&mut categories, board);
        let topology = Topology::after_v14_geometry()?;
        let mut ai_neighbors = crate::ai_neighbors::Cache::default();
        ai_neighbors.refresh(&categories, image)?;
        let (roads, road_segment_count) = board.traffic_roads()?;
        let road_network = crate::traffic::road::construction::RoadNetwork::build(
            &roads,
            road_segment_count,
            &mut categories,
            52,
            &crate::traffic::road::construction::ConstructionSettings::from_v14(image)?,
        )?;
        Ok(Self {
            clock,
            city: City {
                building_parameters: crate::building_parameters::BuildingKinds::from_image(image)?,
                topology,
                road_network,
                objective,
                settings: crate::startup_settings::State::fresh(),
                configuration,
                welfare: crate::resident::Welfare {
                    amounts: [[0.; 5]; 3],
                },
                pause: crate::pause::State::default(),
                buildings,
                ledgers: crate::economy::Ledgers {
                    businesses,
                    residents,
                    players: Vec::new(),
                },
                categories,
                definitions,
                commodities: crate::commodity_script::State::from_v14(image, [0; 5])?,
                catalog: crate::business_script::State::from_v14(image)?,
                creation,
                available: vec![],
                availability_cursor: None,
                player_money: image.f32_at(0x541c78)?,
                construction_components: [0.; 4],
                pending_world: crate::pending_world::State::default(),
                category_footprints: [[0; 2]; 52],
                ai_neighbors,
                ai_demand: crate::ai_demand::State::default(),
                ownership_refresh_counter: 0,
                auction: Default::default(),
            },
        })
    }
    /// Snapshot clock inputs for one script dispatch. The returned context does
    /// not borrow World, allowing subsequent mutable script-state access.
    pub fn script_context<'a, 'image>(
        &self,
        image: &'a crate::pe::PeImage<'image>,
        models: &'a [crate::economy::ResidentModelBinding; 32],
        host: HostContext,
    ) -> Result<level_script::Context<'a, 'image>, String> {
        Ok(level_script::Context {
            precision: host.precision,
            building_parameters: self.city.building_parameters,
            total_clock: self.clock.timing[0],
            image,
            models,
            difficulty: host.difficulty,
            hour: self.clock.hour() as i32,
            discount: image.f32_at(0x53be78)?,
            // Original822958 then822954; conversion preserves low bytes.
            date: [self.clock.calendar[2] as u8, self.clock.calendar[1] as u8],
            timestamp: self.clock.timing[0],
            ownership_timestamp: self.clock.timing[1],
            notifications: host.notifications,
            accounting_enabled: host.accounting_enabled,
            auxiliary_template: host.auxiliary_template,
            category_base: host.category_base,
            building_base: host.building_base,
        })
    }
}
impl World {
    pub fn script_state<'a>(
        &'a mut self,
        players: &'a [Option<crate::business_binding::Player>],
    ) -> level_script::State<'a> {
        self.city.script_state(&mut self.clock, players)
    }
}
impl City {
    /// Borrow current storage for each callback batch; no retained shadow city.
    pub fn script_state<'a>(
        &'a mut self,
        clock: &'a mut crate::clock::Clock,
        players: &'a [Option<crate::business_binding::Player>],
    ) -> level_script::State<'a> {
        level_script::State {
            clock,
            ai_demand: &mut self.ai_demand,
            objective: &mut self.objective,
            settings: &mut self.settings,
            configuration: &mut self.configuration,
            welfare: &mut self.welfare,
            pause: &mut self.pause,
            buildings: &mut self.buildings,
            businesses: self.ledgers.businesses.as_mut_slice(),
            categories: &mut self.categories,
            definitions: &mut self.definitions,
            commodities: &mut self.commodities,
            catalog: &mut self.catalog,
            residents: &mut self.ledgers.residents,
            creation: &mut self.creation,
            available: &mut self.available,
            availability_cursor: &mut self.availability_cursor,
            players,
            player_accounts: &mut self.ledgers.players,
            player_money: &mut self.player_money,
            construction_components: &mut self.construction_components,
        }
    }
}

pub struct Sources<'a> {
    pub default_hub: &'a [u8],
    pub scenario_name: &'a str,
    pub scenario_hub: &'a [u8],
}
/// One VM/event list survives initialization for later level events.
pub struct Scripts {
    pub runtime: Runtime,
    pub bindings: level_script::Bindings,
    pub events: crate::script_events::Events,
}
impl Scripts {
    pub fn new(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let mut runtime = Runtime::new()?;
        let bindings = level_script::Bindings::register(&mut runtime, image)?;
        Ok(Self {
            runtime,
            bindings,
            events: crate::script_events::Events::default(),
        })
    }
    /// Original4d5b40 at its scheduler boundary. Event handlers may append to
    /// the same live list, change the city or pause the host's actual timers.
    pub fn tick_events(
        &mut self,
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
    ) -> Result<(), String> {
        self.runtime
            .tick_events(&mut self.events, &mut |id, args, events| {
                self.bindings
                    .handle(id, args, state, events, context, geometry, hooks)
            })
    }
    pub fn tick_events_with_invocation(
        &mut self,
        mut invoke: impl FnMut(
            &mut Runtime,
            &mut level_script::Bindings,
            crate::script_events::Event,
            &mut crate::script_events::Events,
        ) -> Result<Option<f64>, String>,
    ) -> Result<(), String> {
        let bindings = &mut self.bindings;
        self.runtime
            .tick_events_with_invocation(&mut self.events, &mut |vm, event, events| {
                invoke(vm, bindings, event, events)
            })
    }

    /// Dispatch an optional synchronous CATCHEVENT handler in the retained VM.
    /// Missing handlers are normal (for example SCENARIO3 BlockUnlock). Release
    /// the transient reference even if a host binding fails during the call.
    pub fn dispatch_catch_event(
        &mut self,
        event: (&str, i32),
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
    ) -> Result<Option<f64>, String> {
        Self::dispatch_catch_event_parts(
            &mut self.runtime,
            &mut self.bindings,
            &mut self.events,
            event,
            state,
            context,
            geometry,
            hooks,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_catch_event_parts(
        runtime: &mut Runtime,
        bindings: &mut level_script::Bindings,
        events: &mut crate::script_events::Events,
        event: (&str, i32),
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
    ) -> Result<Option<f64>, String> {
        let name = format!("CATCHEVENT_{}", event.0);
        let Some(reference) = runtime.reference_function(&name)? else {
            return Ok(None);
        };
        let result = runtime.call_reference(reference, event.1, &mut |id, args| {
            bindings.handle(id, args, state, events, context, geometry, hooks)
        });
        let release = runtime.release_reference(reference);
        let result = result?;
        release?;
        Ok(result)
    }

    ///4d5d00 admission through the shared, native-verified Lua trigger binding.
    pub fn trigger_event(
        &mut self,
        event: (&str, u32, i32),
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
    ) -> Result<(), String> {
        if !event
            .0
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("native event name contains unsupported characters".into());
        }
        let source = format!("C_TriggerEvent('{}',{}, {})", event.0, event.1, event.2);
        self.runtime.execute(
            "native event admission",
            source.as_bytes(),
            &mut |id, args| {
                self.bindings
                    .handle(id, args, state, &mut self.events, context, geometry, hooks)
            },
        )
    }

    /// Nested C_RunScript buffers must already be registered on this runtime.
    /// Complete DEFAULT precedes scenario definitions and EVENTINITIALISE_Level.
    pub fn initialize(
        &mut self,
        sources: Sources<'_>,
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
    ) -> Result<(), String> {
        self.initialize_entry(sources, state, context, geometry, hooks, false)
    }

    /// Original4ba07d triggers Level synchronously before48cca0 creates players.
    /// The trigger initializes once and retains any returned event delay.
    pub fn initialize_active(
        &mut self,
        sources: Sources<'_>,
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
    ) -> Result<(), String> {
        self.initialize_entry(sources, state, context, geometry, hooks, true)
    }

    fn initialize_entry(
        &mut self,
        sources: Sources<'_>,
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
        active: bool,
    ) -> Result<(), String> {
        self.runtime
            .execute("DEFAULT/hub.lua", sources.default_hub, &mut |id, args| {
                self.bindings
                    .handle(id, args, state, &mut self.events, context, geometry, hooks)
            })?;
        // Difficulty is applied before level Lua runs. Native loads also
        // reapply the saved selector; the selected income globals are not saved.
        state.settings.apply_difficulty(context.difficulty as usize);
        let mut handler = |id, args: &[crate::lua4::Value]| {
            self.bindings
                .handle(id, args, state, &mut self.events, context, geometry, hooks)
        };
        self.runtime
            .execute(sources.scenario_name, sources.scenario_hub, &mut handler)?;
        if active {
            self.runtime
                .execute("Level entry", b"C_TriggerEvent('Level',0,0)", &mut handler)
        } else {
            self.runtime
                .call("EVENTINITIALISE_Level", &[0.], &mut handler)
        }
    }

    /// Tutorial 2 uses the original Camera table's view callbacks. Keep this
    /// replacement within its retained VM, leaving result Camera.SetMode as-is.
    pub fn bind_tutorial_camera(&mut self) -> Result<(), String> {
        self.runtime.execute("Tutorial camera bindings",
            b"Camera.SetMode=TutorialSetCameraMode; Camera.SetCityView=TutorialSetCityView; Camera.SetPlanView=TutorialSetPlanView",
            &mut |_, _| Err("unexpected tutorial camera binding callback".into()))
    }
    /// Execute the original active-level entry after the roster exists.
    /// Tutorial 1 uses this to establish its first modal and control locks.
    pub fn enter_level(
        &mut self,
        state: &mut level_script::State<'_>,
        context: &level_script::Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl level_script::Hooks,
    ) -> Result<(), String> {
        self.runtime.call("EVENT_Level", &[0.], &mut |id, args| {
            self.bindings
                .handle(id, args, state, &mut self.events, context, geometry, hooks)
        })
    }
}

/// Retained player and AI state after the normal roster stage. Host allocation
/// and ownership/auction callbacks remain explicit; this does not tick the AI.
pub struct Roster {
    pub manager: crate::player_manager::Manager,
    pub profiles: Vec<[u8; 0x650]>,
    pub ai_ownership: crate::ai_ownership::Storage,
    pub category_counts: [u32; 52 * 45],
    pub liquidation_targets: std::collections::BTreeMap<u32, [u8; 12]>,
    /// Original801df0: shared construction/transaction proposal sequence.
    pub proposal_identity: u32,
    pub scripts: crate::ai_script::AiScript,
    pub status: crate::player_script::RosterStatus,
}

pub struct RosterCity<'a> {
    pub categories: &'a [[u8; 0x17c]; 52],
    pub buildings: &'a [[u8; 0x394]],
    pub businesses: &'a [[u8; 0x220]],
    pub clock: &'a crate::clock::Clock,
    pub building_base: u32,
    pub difficulty: i32,
}
impl Roster {
    /// Run the retained normal AI controller against the city's authoritative
    /// accounts. Decision operations not yet bound by ai_runtime stay explicit.
    #[allow(clippy::too_many_arguments)]
    pub fn advance_ai(
        &mut self,
        city: &mut City,
        slot: usize,
        image: &crate::pe::PeImage<'_>,
        controls: &mut crate::ai_tick::Controls,
        allocation_next: &mut u32,
        utilities: &crate::economy::Utilities,
        properties: &crate::economy::PropertyIncome,
        auction: Option<crate::ai_messages::Auction>,
        mut callback: impl FnMut(
            crate::ai_runtime::Call,
            &mut crate::player_manager::Player,
            &mut [[u8; 0x650]],
        ) -> Result<crate::ai_messages::Reply, String>,
    ) -> Result<(), String> {
        self.publish_accounts(city)?;
        let mut world = crate::ai_runtime::World {
            building_parameters: city.building_parameters,
            neighbor_cache: &mut city.ai_neighbors,
            ownership_refresh_counter: &mut city.ownership_refresh_counter,
            colors: None,
            utility_enabled: utilities.enabled,
            liquidation_targets: &mut self.liquidation_targets,
            category_counts: &mut self.category_counts,
            ai_demand_rows: &city.ai_demand.rows,
            active_player_count: crate::player_manager::word(&self.manager.bytes, 12),
            construction_components: &mut city.construction_components,
            construction_allowance: i32::from_le_bytes(image.at(0x541c7c, 4)?.try_into().unwrap()),
            construction_discount: image.f32_at(0x53be78)?,
            affordability_override: false,
            acquisition_checks: false, //4b91b1 startup default.
            proposal_identity: &mut self.proposal_identity,
            residents: &city.ledgers.residents,
            commodity_active: city.commodities.resident_inputs().map(|c| c.active),
            catalog: &city.catalog.info,
            products: &city.catalog.products,
            commodities: &city.commodities.rows,
            category_footprints: &mut city.category_footprints,
            strategy_time: None,
            ownership_tick: None,
            strategy_mode: city.objective.kind,
            business_counts: None,
            message_routing: None,
            categories: &mut city.categories,
            buildings: &city.buildings,
            businesses: &mut city.ledgers.businesses,
            ownership: &mut self.ai_ownership,
            allocation_next,
            global_reset: &mut city.ai_demand.ready,
            income: crate::ai_finance::IncomeTotals::from_economy(properties, utilities),
            auction,
            auction_blocked: auction.is_none().then_some(false),
            auction_guard: None,
            offer_auction: None,
            auction_state: None,
            cash_statuses: &city.objective.cash_statuses,
        };
        let result = crate::ai_runtime::advance(
            slot,
            &mut self.manager,
            &mut self.profiles,
            controls,
            &mut self.scripts,
            image,
            &mut world,
            |call, player, profiles, _| callback(call, player, profiles),
        );
        self.collect_accounts(city)?;
        result
    }

    /// Evaluate against the same live property pools and published accounts
    /// used by history and reports. Host outcome/removal effects remain explicit.
    pub fn evaluate_objective(
        &mut self,
        city: &mut City,
        image: &crate::pe::PeImage<'_>,
        building_base: u32,
        controls: &mut crate::objective_tick::Controls,
        mut callback: impl FnMut(
            crate::objective_tick::Event,
            &mut crate::scenario_objective::State,
            &mut crate::player_manager::Manager,
            &mut crate::objective_tick::Controls,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.publish_accounts(city)?;
        let mut objective = std::mem::take(&mut city.objective);
        let result = crate::objective_tick::advance(
            &mut objective,
            &mut self.manager,
            controls,
            &mut |event, state, manager, controls| {
                if let crate::objective_tick::Event::Metric { address, player } = event {
                    crate::player_metrics::runtime_metric(
                        city,
                        manager.players[player]
                            .bytes
                            .get(4..444)
                            .ok_or("truncated objective account")?
                            .try_into()
                            .unwrap(),
                        player,
                        address,
                        image,
                        building_base,
                    )
                } else {
                    callback(event, state, manager, controls)?;
                    Ok(crate::x87::Sum::from_i32(0))
                }
            },
        );
        city.objective = objective;
        self.collect_accounts(city)?;
        result
    }

    pub fn tick_players(
        &mut self,
        city: &mut City,
        image: &crate::pe::PeImage<'_>,
        controls: &mut crate::player_tick::Controls,
        callback: impl FnMut(
            crate::player_tick::Event,
            &mut crate::player_manager::Manager,
            &mut crate::player_tick::Controls,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.publish_accounts(city)?;
        controls.current_player = self.status.current_player;
        let result = crate::player_tick::advance(&mut self.manager, controls, image, callback);
        self.status.current_player = controls.current_player;
        self.collect_accounts(city)?;
        result
    }

    pub fn account_prefixes(&mut self, city: &City) -> Result<Vec<[u8; 440]>, String> {
        self.manager.account_prefixes(&city.ledgers.players)
    }

    /// Typed accounts are authoritative between manager callbacks. Publish
    /// them before reading raw players; do not silently truncate mismatched
    /// rosters with zip (player removal must reconcile both representations).
    pub fn publish_accounts(&mut self, city: &City) -> Result<(), String> {
        self.manager.publish_accounts(&city.ledgers.players)
    }

    /// Read back after a raw-manager callback, including its partial effects
    /// before an error. Never call this on stale manager accounting records.
    pub fn collect_accounts(&self, city: &mut City) -> Result<(), String> {
        city.ledgers.players = self
            .manager
            .players
            .iter()
            .map(|player| crate::economy::PlayerAccount::from_runtime(&player.bytes))
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    pub fn advance_hourly_account(
        &mut self,
        city: &City,
        slot: Option<usize>,
    ) -> Result<Option<[i32; 8]>, String> {
        self.publish_accounts(city)?;
        let count = self.manager.players.len() as i32;
        let slot = match slot {
            Some(slot) => slot,
            None => self
                .manager
                .players
                .iter()
                .position(|player| player.reference == self.status.current_player)
                .ok_or("current hourly player is unresolved")?,
        };
        self.manager
            .players
            .get_mut(slot)
            .ok_or("hourly player leaves roster")?
            .advance_hourly_account(count)
    }

    pub fn update_cash_status(
        &mut self,
        city: &mut City,
        hour: u32,
        controls: &mut crate::player_manager::CashControls,
        callback: impl FnMut(
            crate::player_manager::CashEvent,
            &mut crate::player_manager::Manager,
            &mut [u8; 6],
            &mut crate::player_manager::CashControls,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.publish_accounts(city)?;
        let result = self.manager.update_cash_status(
            hour,
            &mut city.objective.cash_statuses,
            controls,
            callback,
        );
        self.collect_accounts(city)?;
        result
    }

    ///470b80/466bf0/46f020 over the retained AI allocations and event auxiliaries.
    /// This pass runs on every peer, unlike periodic AI controllers and hourly
    /// event-receiver ticks. Account publication/readback is shared work.
    pub fn advance_hourly_ai(
        &mut self,
        city: &mut City,
        image: &crate::pe::PeImage<'_>,
        hour: u32,
        mut callback: impl FnMut(
            usize,
            crate::ai_hourly::UpdateCall,
            &mut crate::player_manager::Player,
            &City,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.publish_accounts(city)?;
        let result = crate::ai_hourly::advance(
            &mut self.manager.players,
            hour,
            &mut crate::ai_hourly::Callbacks {
                update: &mut |call, player| {
                    crate::ai_hourly::update(
                        player,
                        &city.objective.cash_statuses,
                        image,
                        |call2, player| callback(call.player, call2, player, city),
                    )
                },
                delay: &mut |_| {
                    Ok(u32::from(crate::ai_hourly::delay_signal(
                        city.objective.kind as u32,
                    )))
                },
            },
        );
        self.collect_accounts(city)?;
        result
    }

    /// Original4899e0 against the same accounts used by residents/businesses.
    /// Inactive native stack slots require an explicit host prior-byte policy.
    pub fn advance_market_share(
        &mut self,
        city: &mut City,
        day: u32,
        network: [bool; 2],
        prior: [f32; 6],
        mut callback: impl FnMut(
            crate::player_share::Call,
            &mut crate::player_manager::Manager,
        ) -> Result<crate::player_share::CallbackEffect, String>,
        mut outcome: impl FnMut(
            crate::objective_tick::Event,
            &mut crate::scenario_objective::State,
            &mut crate::player_manager::Manager,
            &mut crate::objective_tick::Controls,
        ) -> Result<crate::x87::Sum, String>,
    ) -> Result<crate::player_share::ResultState, String> {
        self.publish_accounts(city)?;
        let current_slot = self
            .manager
            .players
            .iter()
            .position(|p| p.reference == self.status.current_player)
            .ok_or("market-share current player is unresolved")?;
        let controls = crate::player_share::Controls {
            manager_kind: city.objective.kind,
            manager_value: city.objective.target,
            day,
            player_count: self.manager.players.len(),
            network_enabled: network[0],
            network_override: network[1],
            current_slot,
        };
        let mut descriptor = city.objective.to_bytes();
        let mut players: Vec<_> = std::mem::take(&mut self.manager.players)
            .into_iter()
            .map(Some)
            .collect();
        let result = crate::player_share::apply_with_state_callback(
            controls,
            Some(&mut descriptor),
            prior,
            &mut players,
            |call, share_controls, descriptor, players| {
                let descriptor = descriptor.ok_or("market-share outcome descriptor missing")?;
                let mut state = crate::scenario_objective::State::from_bytes(descriptor)?;
                let mut manager = crate::player_manager::Manager {
                    bytes: self.manager.bytes,
                    players: players.iter_mut().filter_map(Option::take).collect(),
                };
                let mut controls = crate::objective_tick::Controls {
                    network_enabled: share_controls.network_enabled,
                    network_override: share_controls.network_override,
                    mode: u32::from_le_bytes(manager.bytes[12..16].try_into().unwrap()),
                    year: 0,
                    day,
                };
                let result = if let crate::player_share::Call::LateController { argument } = call {
                    crate::objective_tick::resolve(
                        &mut state,
                        &mut manager,
                        &mut controls,
                        argument,
                        &mut outcome,
                    )
                    .map(|_| crate::player_share::CallbackEffect::default())
                } else {
                    callback(call, &mut manager)
                };
                self.manager.bytes = manager.bytes;
                let count_ok = manager.players.len() == players.len();
                for (destination, player) in players.iter_mut().zip(manager.players) {
                    *destination = Some(player);
                }
                descriptor.copy_from_slice(&state.to_bytes());
                share_controls.manager_value = state.target;
                share_controls.network_enabled = controls.network_enabled;
                share_controls.network_override = controls.network_override;
                let effect = result?;
                if !count_ok {
                    return Err("outcome callback changed allocated roster slots".into());
                }
                Ok(effect)
            },
        );
        // Preserve partial native writes even when a host boundary fails.
        self.manager.players = players.into_iter().flatten().collect();
        city.objective = crate::scenario_objective::State::from_bytes(&descriptor)?;
        self.collect_accounts(city)?;
        result
    }

    /// Fresh, script-created city roster with no ownership allocation boundary.
    /// Existing zero-owned callers retain their bounded startup behavior.
    pub fn initialize_world(
        &mut self,
        pending: &crate::player_script::PendingRoster,
        inputs: crate::player_script::RosterInputs<'_>,
        construction: &mut crate::player_constructor::RosterConstruction<'_>,
        world: &mut World,
        difficulty: i32,
    ) -> Result<(), String> {
        self.initialize_world_with_ownership_allocator(
            pending,
            inputs,
            construction,
            world,
            difficulty,
            |_| Err("fresh roster ownership requires a host allocation boundary".into()),
        )
    }

    /// Fresh roster with authored ownership. Run original467a30/4708a0 against
    /// the live city and script-mutated catalog immediately after each actual
    /// AI reset, in initialize_created's original order. Allocation identities
    /// must share the host's roster/runtime arena.
    pub fn initialize_world_with_ownership_allocator(
        &mut self,
        pending: &crate::player_script::PendingRoster,
        inputs: crate::player_script::RosterInputs<'_>,
        construction: &mut crate::player_constructor::RosterConstruction<'_>,
        world: &mut World,
        difficulty: i32,
        mut allocate_ownership: impl FnMut(usize) -> Result<u32, String>,
    ) -> Result<(), String> {
        let image = construction.image;
        // Temporarily detach storage so initialize can borrow the roster. Always
        // restore it before propagating errors, preserving partial native writes.
        let mut ownership = std::mem::take(&mut self.ai_ownership);
        let result = self.initialize(
            pending,
            inputs,
            construction,
            RosterCity {
                categories: &world.categories,
                buildings: &world.buildings,
                businesses: &world.ledgers.businesses,
                clock: &world.clock,
                building_base: 0x591318,
                difficulty,
            },
            &mut |manager, reference| {
                let player = manager
                    .players
                    .iter_mut()
                    .find(|p| p.reference == reference)
                    .ok_or("fresh ownership player is unresolved")?;
                crate::ai_ownership::maintain_player(
                    &mut ownership,
                    player,
                    &world.categories,
                    &world.buildings,
                    &world.ledgers.businesses,
                    image,
                    &world.catalog.info,
                    &mut allocate_ownership,
                )
            },
        );
        self.ai_ownership = ownership;
        result?;
        world.ledgers.players = self
            .manager
            .players
            .iter()
            .map(|p| crate::economy::PlayerAccount::from_runtime(&p.bytes))
            .collect::<Result<_, _>>()?;
        Ok(())
    }
    /// Original immediate post-roster52-category pass, before the ownership
    /// accounting gate is enabled. City-owned rows resolve to null and skip it.
    pub fn initialize_ownership(
        &mut self,
        world: &mut World,
        image: &crate::pe::PeImage<'_>,
        seams: &mut impl FnMut(
            &mut crate::category_ownership::World<'_>,
            crate::category_ownership::Seam,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        let chains =
            crate::building_pool::category_chains(&world.categories, &world.buildings, 0x591318)?;
        let current_player = self
            .manager
            .players
            .iter()
            .position(|p| p.reference == self.status.current_player);
        let normal_tick = world.clock.timing[1];
        let city = &mut world.city;
        let mut ownership = crate::category_ownership::World {
            categories: &mut city.categories,
            buildings: &mut city.buildings,
            businesses: &mut city.ledgers.businesses,
            players: &mut self.manager.players,
            chains: &chains,
            normal_tick,
            category_tick: city.settings.lease_length() as u32,
            refresh_counter: city.ownership_refresh_counter,
            accounting_enabled: false,
            current_player,
            notification_threshold: image.f32_at(0x525040)?,
        };
        let result = ownership.fresh_loop(seams);
        city.ownership_refresh_counter = ownership.refresh_counter;
        // Publish partial native accounting effects even if a host seam failed.
        city.ledgers.players = ownership
            .players
            .iter()
            .map(|p| crate::economy::PlayerAccount::from_runtime(&p.bytes))
            .collect::<Result<_, _>>()?;
        result
    }

    /// Manager constructor preserves unspecified bytes supplied by the caller.
    pub fn new(manager_prior_bytes: [u8; 0x28]) -> Result<Self, String> {
        Ok(Self {
            manager: crate::player_manager::Manager::initialize(manager_prior_bytes),
            profiles: vec![[0; 0x650]; 10],
            ai_ownership: crate::ai_ownership::Storage::default(),
            category_counts: [0; 52 * 45],
            liquidation_targets: Default::default(),
            proposal_identity: 0,
            scripts: crate::ai_script::AiScript::new()?,
            status: crate::player_script::RosterStatus::default(),
        })
    }

    /// Normal48cca0 followed by4d7a80, sharing the populated city's records.
    /// Errors retain partial construction effects for the host to inspect.
    pub fn initialize(
        &mut self,
        pending: &crate::player_script::PendingRoster,
        inputs: crate::player_script::RosterInputs<'_>,
        construction: &mut crate::player_constructor::RosterConstruction<'_>,
        city: RosterCity<'_>,
        after_reset: &mut impl FnMut(&mut crate::player_manager::Manager, u32) -> Result<(), String>,
    ) -> Result<(), String> {
        let chains = crate::building_pool::category_chains(
            city.categories,
            city.buildings,
            city.building_base,
        )?;
        let precision = inputs.precision;
        pending.initialize_created(
            inputs,
            construction,
            &mut crate::ai_script::Initialization {
                precision,
                profiles: &mut self.profiles,
                manager: &mut self.manager,
                category_counts: &mut self.category_counts,
                chains: &chains,
                buildings: city.buildings,
                businesses: city.businesses,
                difficulty: city.difficulty,
                current_hour: city.clock.hour() as i32,
                count_tick: Some(city.clock.timing[1]),
            },
            &mut self.scripts,
            &mut self.status,
            after_reset,
        )
    }
}

#[cfg(test)]
mod source_epoch_tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    #[test]
    fn topology_epoch_mint_is_checked_and_exhaustion_does_not_reuse_identity() {
        // Local counter exercises the SAME production mint operation without
        // consuming or racing the process's World identity domain.
        let next = AtomicU64::new(u64::MAX - 1);
        assert_eq!(mint_source_epoch(&next).unwrap().serial(), u64::MAX - 1);
        assert_eq!(
            mint_source_epoch(&next).unwrap_err(),
            "World source epoch exhausted"
        );
        assert_eq!(
            mint_source_epoch(&next).unwrap_err(),
            "World source epoch exhausted"
        );
    }
    #[test]
    fn topology_completion_retains_native_count_and_distinct_epoch() {
        let first = Topology::after_v14_geometry().unwrap();
        let second = Topology::after_v14_geometry().unwrap();
        assert_eq!((first.count617624(), second.count617624()), (52, 52));
        assert!(second.epoch() > first.epoch());
    }
}
