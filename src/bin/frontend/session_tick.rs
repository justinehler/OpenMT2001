//! Retained single-player dispatch. Unbound host/system work is returned
//! synchronously, so callers must handle it or stop with an explicit error.
use super::{CommandAssets, Session};
use openmt2001::{
    business_binding, economy, hourly,
    level_runtime::{CalendarEvent, HourlyContext},
    level_startup, live_commands, scheduler,
};

pub struct Policy {
    pub scheduler: scheduler::Controls,
    pub calendar: openmt2001::clock::Controls,
    pub hourly: hourly::Controls,
    pub camera: [f32; 3],
    pub distance_disabled: bool,
    pub notifications: bool,
    pub effect_model: u32,
    /// Explicit policy for original uninitialized inactive market-share locals.
    pub market_share_prior: [f32; 6],
    pub player_message_prior: [u8; 48],
    /// Original auction bidder panel565b9c+858, toggled by414a31. This is
    /// independent of card/modal visibility and remainsfalse until its control is bound.
    pub ai_event_gate: bool,
}

/// Both dispatchers refer to the original retained cue at 0x822964. Preserve
/// stores made before a failed hourly boundary, just like other live controls.
pub(crate) fn advance_scheduled_hour(
    scheduler: &mut scheduler::Controls,
    context: &mut HourlyContext<'_>,
    advance: impl FnOnce(&mut scheduler::Controls, &mut HourlyContext<'_>) -> Result<(), String>,
) -> Result<(), String> {
    context.hourly.before_six_sound = scheduler.before_six_sound;
    let result = advance(scheduler, context);
    scheduler.before_six_sound = context.hourly.before_six_sound;
    result
}
#[derive(Debug)]
pub enum Event<'a> {
    ScriptCommands(
        &'a [(
            openmt2001::level_script::Command,
            Vec<openmt2001::lua4::Value>,
        )],
    ),
    Tick(scheduler::Event),
    Calendar(CalendarEvent),
    AccountPosted(economy::AccountPosting),
    Autosave(&'a mut dyn crate::session_save::AutosaveRequest),
    Command(live_commands::Event),
    Pending(live_commands::PendingCall, Option<&'a mut [u8; 0x394]>),
    Player(openmt2001::player_tick::Event),
    TransportSound(openmt2001::traffic::RouteSound),
    EraMusic {
        era: u32,
        repeat: bool,
    },
    TransportEnvironment {
        call: openmt2001::traffic::scene::EnvironmentCall,
        position: [f32; 3],
    },
    MarketShare(openmt2001::player_share::Call),
    HumanDialog {
        call: openmt2001::human_messages::Call,
        name: Option<[u8; 96]>,
    },
    Ai(openmt2001::ai_runtime::Call),
    Script {
        name: String,
        argument: i32,
        commands: &'a [(
            openmt2001::level_script::Command,
            Vec<openmt2001::lua4::Value>,
        )],
    },
}

impl Session<'_> {
    /// Poll admitted ingress before the host's pause/timer gate.
    pub fn pump_connected_commands(&mut self, policy: &mut Policy) -> Result<(), String> {
        let Some(connection) = &mut self.connected else {
            return Ok(());
        };
        let tick = self.world.clock.timing[0];
        let targets = openmt2001::network::commands::Targets::new(
            &self.world.city.buildings,
            &self.world.city.ledgers.businesses,
            &self.world.city.categories,
        );
        connection.commands.pump(
            &connection.worker,
            tick,
            &mut self.live_commands,
            &mut self.world.city.categories,
            targets,
            &mut self.world.city.pause,
            &mut self.hooks.timers,
        )?;
        policy.scheduler.network_enabled = true;
        policy.scheduler.network_override = connection.commands.is_host();
        connection.commands.configure_hourly(&mut policy.hourly);
        policy.scheduler.deadline = connection.commands.deadline();
        Ok(())
    }
    /// Retained group-0 callback 0x450410. Read human activity from the live
    /// manager on each pass; AI and eliminated rows never hold the barrier.
    pub fn pump_connected_synchronization(&mut self, policy: &mut Policy) -> Result<bool, String> {
        self.pump_connected_commands(policy)?;
        let Some(connection) = &mut self.connected else {
            return Ok(false);
        };
        let active =
            openmt2001::network::synchronization::active_humans(&self.roster.manager.players);
        connection.synchronization.pump(
            &connection.worker,
            active,
            &mut self.world.city.pause,
            &mut self.hooks.timers,
        )
    }
    /// One original4ba580 interface tick, independent of25Hz logic dispatch.
    /// The caller must drive its registered50Hz task; paused logic alone does
    /// not suppress interface dispatch. Only retained8b22d8 gates the countdown.
    pub fn advance_interface_tick(
        &mut self,
        iterations: u32,
        suppressed: bool,
        mut call: impl FnMut(
            openmt2001::ai_messages::offers::auction::announcement::Call,
        ) -> Result<u32, String>,
    ) -> Result<(), String> {
        let exists = self.world.city.auction.controller.reference != 0;
        let paused = self.world.city.pause.bytes[8] != 0;
        if suppressed || self.world.city.pause.bytes[3] as i8 > 0 {
            return Ok(());
        }
        for _ in 0..iterations {
            if self.world.city.auction.presentation.visible {
                openmt2001::ai_messages::offers::auction::announcement::advance(
                    &mut self.world.city.auction.presentation,
                    exists,
                    paused,
                    |event, _| call(event),
                )?;
            }
        }
        Ok(())
    }
    /// Original scene frame processing is separate from normal logic ticks.
    /// A paused host may still request a frame at the retained simulation time.
    pub fn advance_transport_frame(
        &mut self,
        submit: impl FnMut(&mut [u8; 284]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.transport.advance_frame(
            self.world.clock.timing[1],
            &self.transport_models,
            self.policy.auxiliary_prior_bytes,
            submit,
        )
    }
    /// Apply an original script event (including prompt dismissal) to the same
    /// paused or running session, restoring retained geometry even on failure.
    pub fn dispatch_session_event(
        &mut self,
        name: &str,
        argument: i32,
        assets: &CommandAssets<'_, '_>,
        _notifications: bool,
    ) -> Result<(), String> {
        let closes_script = name == "InterfacePromptDismiss"
            && self
                .hooks
                .script_prompt()
                .is_some_and(|r| r.dismissal_id == argument as u32);
        let context = self.world.script_context(
            assets.image,
            &self.resident_models,
            level_startup::HostContext {
                precision: openmt2001::x87::Precision::Pc64,
                accounting_enabled: true,
                difficulty: self.difficulty as i32,
                notifications: self.world.city.ai_demand.ready != 0,
                auxiliary_template: self.policy.auxiliary_prior_bytes,
                category_base: 0x617628,
                building_base: 0x591318,
            },
        )?;
        let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
            assets.board,
            assets.file,
            std::mem::take(&mut self.models),
        );
        let result = (|| {
            for index in 0..geometry.registrar().models.len() {
                geometry.retain_model(
                    openmt2001::building_geometry::RegisteredModel::from_definition(
                        index as u32 + 1,
                        &geometry.registrar().models[index].definition,
                    )?,
                )?;
            }
            let players: Vec<_> = self
                .roster
                .manager
                .players
                .iter()
                .map(|p| {
                    Some(business_binding::Player {
                        identity: p.reference,
                        kind: u32::from_le_bytes(p.bytes[0x154..0x158].try_into().unwrap()),
                    })
                })
                .collect();
            self.scripts.dispatch_catch_event(
                (name, argument),
                &mut self.world.script_state(&players),
                &context,
                &mut geometry,
                &mut self.hooks,
            )
        })();
        self.models = std::mem::take(geometry.registrar_mut());
        let refreshed = self
            .population
            .refresh_city(&self.world.city, assets.board)
            .and_then(|()| {
                self.population.refresh_bindings_if_changed(
                    &self.world.city,
                    &self.resident_models,
                    &self.people,
                )
            });
        if closes_script {
            self.hooks.close_script_prompt(false);
        }
        result.and(refreshed)
    }
    /// Run the original normal logic ordering over the retained session. Host
    /// effects are synchronous; a failing boundary preserves preceding stores.
    /// A script pause stops future calls, including delayed command draining.
    pub fn advance_session(
        &mut self,
        assets: &CommandAssets<'_, '_>,
        policy: &mut Policy,
        boundary: impl FnMut(Event<'_>, &openmt2001::clock::Clock) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.advance_session_with_animations(assets, policy, || Ok(()), boundary)
    }
    pub fn advance_session_with_animations(
        &mut self,
        assets: &CommandAssets<'_, '_>,
        policy: &mut Policy,
        animations: impl FnMut() -> Result<(), String>,
        mut boundary: impl FnMut(Event<'_>, &openmt2001::clock::Clock) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.pump_connected_commands(policy)?;
        //481718..481736 keeps the level script countdown active in auction
        //mode when the retained selected level object's+85c byte is nonzero.
        policy.scheduler.alternate_script_active = self.tutorial_state != 0;
        if self.world.pause.has_reason() {
            return Ok(());
        }
        let image = assets.image;
        let board = assets.board;
        let messages = std::cell::RefCell::new(openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena {
                next: self.allocation_next,
            },
            current_player: self.roster.status.current_player,
            network: [
                policy.scheduler.network_enabled,
                policy.scheduler.network_override,
            ],
            prior: policy.player_message_prior,
        });
        let mut script_context = self.world.script_context(
            image,
            &self.resident_models,
            level_startup::HostContext {
                precision: openmt2001::x87::Precision::Pc64,
                accounting_enabled: true,
                difficulty: self.difficulty as i32,
                notifications: self.world.city.ai_demand.ready != 0,
                auxiliary_template: self.policy.auxiliary_prior_bytes,
                category_base: 0x617628,
                building_base: 0x591318,
            },
        )?;
        let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
            board,
            assets.file,
            std::mem::take(&mut self.models),
        );
        let mut utilities =
            std::mem::replace(&mut self.hooks.utilities, economy::Utilities::new([0; 4]));
        let requested_iterations = policy.scheduler.iterations;
        let admitted_iterations = if self.connected.is_some() {
            requested_iterations.min(
                policy
                    .scheduler
                    .deadline
                    .saturating_sub(self.world.clock.timing[0]),
            )
        } else {
            requested_iterations
        };
        policy.scheduler.iterations = admitted_iterations;
        let result = (|| {
            for index in 0..geometry.registrar().models.len() {
                geometry.retain_model(
                    openmt2001::building_geometry::RegisteredModel::from_definition(
                        index as u32 + 1,
                        &geometry.registrar().models[index].definition,
                    )?,
                )?;
            }
            let mut context = HourlyContext {
                board,
                calendar: policy.calendar,
                hourly: policy.hourly,
                utilities: &mut utilities,
            };
            let routes = openmt2001::traffic::road::routes::RoadRoutes::from_bytes(
                &self.road_routes,
                self.world.road_network.road_segment_count as u32,
            )?;
            let mut dispatch = openmt2001::level_runtime::Dispatch {
                population: &mut self.population,
                roads: &mut self.road_traffic,
                routes: &routes,
                models: &mut self.vehicle_models,
                camera: policy.camera,
                distance_disabled: policy.distance_disabled,
            };
            let result = dispatch.advance_with_animations(
                &mut self.world,
                &mut policy.scheduler,
                animations,
                |event, clock, city, scheduler, dispatch| {
                    let event = match event {
                        openmt2001::level_runtime::Event::AccountPosted(posting) => {
                            boundary(Event::AccountPosted(posting), clock)?;
                            return Ok(0);
                        }
                        openmt2001::level_runtime::Event::Scheduled(event) => event,
                    };
                    // Original4ed6f0 is mov al,1;ret.4ed6e0 selects PC64;
                    // our shared numeric routines model that precision explicitly.
                    if matches!(event, scheduler::Event::Helper(0x4ed6f0 | 0x4ed6e0)) {
                        return Ok(u32::from(matches!(event, scheduler::Event::Helper(0x4ed6f0))));
                    }
                    // Scheduler emits this after incrementing Total, before
                    // command draining, resident movement or any world mutation.
                    if matches!(event, scheduler::Event::Helper(0x481340)) {
                        if let Some(connection) = &mut self.connected {
                            connection.synchronization.capture(&connection.worker, clock.timing[0], city, dispatch.roads, &self.roster.manager.players)?;
                        }
                        return Ok(0);
                    }
                    if matches!(event, scheduler::Event::Helper(0x4bac90)) && self.connected.is_some() {
                        openmt2001::network::commands::pause_at_limit(&mut city.pause,&mut self.hooks.timers)?;
                        return Ok(0);
                    }
                    match event {
                        scheduler::Event::Helper(0x4eb5b0) => {
                            self.demolition_effects.advance(image)?;
                            self.demolition_points=self.demolition_effects.points(image)?;
                        }
                        scheduler::Event::AdvanceHour => {
                            advance_scheduled_hour(scheduler, &mut context, |scheduler, context|
                            dispatch.population.advance_hour(
                                clock,
                                city,
                                context,
                                |event, clock, city, population, context| {
                                    match event {
                                        CalendarEvent::AccountPosted(posting) => {
                                            boundary(Event::AccountPosted(posting), clock)?;
                                        }
                                        //48dbf0 returns the retained level object's+85c byte.
                                        CalendarEvent::Hourly(hourly::Event::Query) | CalendarEvent::Clock(openmt2001::clock::Event::RolloverQuery) => {
                                            return Ok(self.tutorial_state != 0);
                                        }
                                        CalendarEvent::Clock(openmt2001::clock::Event::Era(requested)) => {
                                            openmt2001::era::advance(requested as i32,clock,|call,clock|{
                                                match call {
                                                    openmt2001::era::Call::Parameters(year)=>{
                                                        let index=year.wrapping_sub(1930)/10;
                                                        // All eight original era files contain only TexturePrestigeRanking
                                                        // and begin with RESET; these are their native-verified parsed banks.
                                                        self.building_styles=self.era_styles.get(index as usize).ok_or("era parameter year leaves original assets")?.clone();
                                                    },
                                                    openmt2001::era::Call::Music{era,repeat}=>{boundary(Event::EraMusic{era,repeat},clock)?;},
                                                    openmt2001::era::Call::Script{name,delay,argument}=>{
                                                        let raw=image.at(name,32)?;let n=raw.iter().position(|b|*b==0).ok_or("era event name unterminated")?;
                                                        let name=std::str::from_utf8(&raw[..n]).map_err(|e|e.to_string())?;
                                                        script_context.update_clock(clock);
                                                        let players:Vec<_>=self.roster.manager.players.iter().map(|p|Some(business_binding::Player{identity:p.reference,kind:u32::from_le_bytes(p.bytes[0x154..0x158].try_into().unwrap())})).collect();
                                                        std::mem::swap(context.utilities,&mut self.hooks.utilities);
                                                        let first=self.hooks.commands.len();
                                                        let result=self.scripts.trigger_event((name,delay,argument),&mut city.script_state(clock, &players),&script_context,&mut geometry,&mut self.hooks);
                                                        std::mem::swap(context.utilities,&mut self.hooks.utilities); script_context.update_clock(clock); result?;
                                                        population.refresh_bindings_if_changed(city,&self.resident_models,&self.people)?;
                                                        boundary(Event::Script{name:name.to_owned(),argument,commands:&self.hooks.commands[first..]},clock)?;
                                                    },
                                                }Ok(())
                                            })?;
                                        }
                                        CalendarEvent::Clock(openmt2001::clock::Event::Autosave) => {
                                            let projection=crate::session_save::SaveProjection {
                                                live_commands: &self.live_commands,
                                                city: city,
                                                clock: clock,
                                                utilities: context.utilities,
                                                frequency: &self.hooks.frequency,
                                                timers: &self.hooks.timers,
                                                scripts: &mut self.scripts,
                                                roster: &mut self.roster,
                                                population: population,
                                                road_traffic: dispatch.roads,
                                                transport: &self.transport,
                                                catalog: &self.catalog,
                                                campaign_profile: &self.campaign_profile,
                                                skirmish_map: &self.skirmish_map,
                                                saved_player_metadata: &self.saved_player_metadata,
                                                history: &self.history,
                                                random_registry: &self.random_registry,
                                                event_cards: &self.event_cards,
                                                pulse_lights: &self.pulse_lights,
                                                demolition_effects: &self.demolition_effects,
                                                policy: &self.policy,
                                                session_option_word: self.session_option_word,
                                                difficulty: self.difficulty,
                                                tutorial_state: self.tutorial_state,
                                                result_present: self.result_presentation.is_some(),
                                                script_random_state: self.script_random_state,
                                                credit_random_state: self.credit_random_state,
                                                particle_random_state: self.particle_random_state,
                                                allocation_next: messages.borrow().arena.next,
                                                static_rng_tail_next: self.static_rng_tail_next,
                                                build_random_state: self.build_random_state,
                                                building_selection: self.building_selection,
                                            };
                                            let mut request=crate::session_save::SaveRequest {projection:Some(projection),image};
                                            boundary(Event::Autosave(&mut request),clock)?;
                                        }
                                        CalendarEvent::Clock(openmt2001::clock::Event::Rollover) => {
                                            openmt2001::ai_daily::advance(&mut self.roster.manager.players,&mut self.roster.ai_ownership)?;
                                        }
                                        CalendarEvent::Hourly(hourly::Event::Script {
                                            name,
                                            argument,
                                        }) => {
                                            let bytes = image.at(name, 32)?;
                                            let end = bytes
                                                .iter()
                                                .position(|&b| b == 0)
                                                .ok_or("hourly event name unterminated")?;
                                            let name = std::str::from_utf8(&bytes[..end])
                                                .map_err(|e| e.to_string())?;
                                            script_context.update_clock(clock);
                                            let players: Vec<_> = self
                                                .roster
                                                .manager
                                                .players
                                                .iter()
                                                .map(|p| {
                                                    Some(business_binding::Player {
                                                        identity: p.reference,
                                                        kind: u32::from_le_bytes(
                                                            p.bytes[0x154..0x158]
                                                                .try_into()
                                                                .unwrap(),
                                                        ),
                                                    })
                                                })
                                                .collect();
                                            std::mem::swap(
                                                context.utilities,
                                                &mut self.hooks.utilities,
                                            );
                                            let command_start = self.hooks.commands.len();
                                            let result = self.scripts.dispatch_catch_event(
                                                (name, argument as i32),
                                                &mut city.script_state(clock, &players),
                                                &script_context,
                                                &mut geometry,
                                                &mut self.hooks,
                                            );
                                            std::mem::swap(
                                                context.utilities,
                                                &mut self.hooks.utilities,
                                            );
                                            script_context.update_clock(clock);
                                            result?;
                                            population.refresh_bindings_if_changed(city,&self.resident_models,&self.people)?;
                                            boundary(
                                                Event::Script {
                                                    name: name.to_owned(),
                                                    argument: argument as i32,
                                                    commands: &self.hooks.commands[command_start..],
                                                },
                                                clock,
                                            )?;
                                        }
                                        CalendarEvent::Hourly(hourly::Event::Categories) => {
                                            let result=live_commands::expire_leases(city,&mut self.roster,&mut messages.borrow_mut(),
                                                &live_commands::Context{image,clock,accounting_enabled:true,notifications:city.ai_demand.ready!=0,
                                                    tutorial_state:self.tutorial_state,network_enabled:scheduler.network_enabled,network_override:scheduler.network_override,
                                                    utility_factor:1.},|event|{
                                                        if matches!(event,live_commands::Event::LeaseNotification(openmt2001::category::Call{address:0x43be20,..}))&&!scheduler.network_enabled{scheduler.iterations=1;}
                                                        boundary(Event::Command(event),clock).map(|_|())
                                                    });
                                            let refresh=population.refresh_city(city,board);
                                            result.and(refresh)?;
                                        }
                                        CalendarEvent::Hourly(hourly::Event::Helper(0x4895b0)) => {
                                            messages.borrow_mut().current_player=self.roster.status.current_player;
                                            self.roster.update_cash_status(
                                                city,
                                                clock.hour(),
                                                &mut openmt2001::player_manager::CashControls {
                                                    network_enabled: scheduler.network_enabled,
                                                    network_override: scheduler.network_override,
                                                },
                                                |event, manager, _, _| {
                                                    live_commands::queue_cash_status(event,&mut self.live_commands,clock.timing[0],manager,&mut messages.borrow_mut(),
                                                        |call|boundary(Event::Command(call),clock).map(|_|()))
                                                },
                                            )?;
                                        }
                                        CalendarEvent::Hourly(
                                            hourly::Event::CurrentPlayerUpdate | hourly::Event::PlayerUpdate(_),
                                        ) => {
                                            let slot=match event { CalendarEvent::Hourly(hourly::Event::PlayerUpdate(slot))=>Some(slot), _=>None };
                                            if let Some(command) =
                                                self.roster.advance_hourly_account(city, slot)?
                                            {
                                                let mut card_request=None;
                                                messages.borrow_mut().produce(command,&mut self.roster.manager,|route,packet| {
                                                    if route==openmt2001::player_messages::Route::Broadcast && matches!(u32::from_le_bytes(packet[4..8].try_into().unwrap()),0x31|0x32|0x34) {
                                                        card_request=Some(*packet);Ok(())
                                                    } else {boundary(Event::Command(live_commands::Event::PlayerMessageRoute{route,packet:*packet}),clock).map(|_|())}
                                                })?;
                                                if let Some(packet)=card_request {
                                                    self.event_cards.broadcast(&packet,clock.calendar[4],image,|card,_| {
                                                        messages.borrow_mut().reply(&packet,0x33,[card,0x5f5e0ff,0x5f5e0ff,0x5f5e0ff],&mut self.roster.manager,
                                                            |route,packet|boundary(Event::Command(live_commands::Event::PlayerMessageRoute{route,packet:*packet}),clock).map(|_|())).map(|_|())
                                                    })?;
                                                }

                                            }
                                        }
                                        CalendarEvent::Hourly(hourly::Event::Helper(0x4899e0)) => {
                                            openmt2001::player_share::observer::before(
                                                &self.roster.manager, city, self.roster.status.current_player,
                                                clock.calendar[4], policy.market_share_prior,
                                                [scheduler.network_enabled, scheduler.network_override]);
                                            let boundary = std::cell::RefCell::new(&mut boundary);
                                            let queue = std::cell::RefCell::new(&mut self.live_commands);
                                            messages.borrow_mut().current_player = self.roster.status.current_player;
                                            self.roster.advance_market_share(
                                                city, clock.calendar[4],
                                                [scheduler.network_enabled, scheduler.network_override],
                                                policy.market_share_prior,
                                                |call, manager| {
                                                    use openmt2001::player_share::Call;
                                                    match call {
                                                        Call::CommandProducer {slot} | Call::RemovePlayer {slot} => {
                                                            let event = if matches!(call,Call::CommandProducer{..}) {
                                                                openmt2001::objective_tick::Event::Outcome{winner:slot as i32,reason:1}
                                                            } else {openmt2001::objective_tick::Event::RemovePlayer(slot as usize)};
                                                            live_commands::queue_outcome(event, &mut queue.borrow_mut(), clock.timing[0], manager, &mut messages.borrow_mut(),
                                                                |call| boundary.borrow_mut()(Event::Command(call), clock).map(|_|()))?;
                                                            Ok(Default::default())
                                                        }
                                                        Call::Command {args} => {
                                                            live_commands::player_message(args.map(|v|v as i32),manager,&mut messages.borrow_mut(),
                                                                |call| boundary.borrow_mut()(Event::Command(call), clock).map(|_|()))?;
                                                            Ok(Default::default())
                                                        }
                                                        Call::LateController {..} => Err("late outcome must be resolved by retained roster".into()),
                                                        _ => {
                                                            boundary.borrow_mut()(Event::MarketShare(call), clock)?;
                                                            Ok(Default::default())
                                                        }
                                                    }
                                                },
                                                |event, _, manager, _| {
                                                    live_commands::queue_outcome(event, &mut queue.borrow_mut(), clock.timing[0], manager, &mut messages.borrow_mut(),
                                                        |call| boundary.borrow_mut()(Event::Command(call), clock).map(|_|()))?;
                                                    Ok(openmt2001::x87::Sum::from_i32(0))
                                                },
                                            )?;
                                            openmt2001::player_share::observer::after(
                                                &self.roster.manager, city);
                                        }
                                        CalendarEvent::Hourly(hourly::Event::Helper(0x470b80)) => {
                                            use openmt2001::event_cards;
                                            let word=|b:&[u8],at:usize|u32::from_le_bytes(b[at..at+4].try_into().unwrap());
                                            let players:Vec<_>=self.roster.manager.players.iter().map(|p|p.reference).collect();
                                            let admitted=word(&self.roster.manager.bytes,12)!=0;
                                            // Pointer-free handles for the original category scene nodes.
                                            // The board owns category identity; handles persist across hours.
                                            let nodes:Vec<_>=board.blocks.iter().map(|b|event_cards::lifecycle::Node {
                                                reference:0xe0000000+b.id as u32*284,kind:4,category:b.id as u32,
                                            }).collect();
                                            let mut outgoing_cards = Vec::new();
                                            self.roster.advance_hourly_ai(city,image,clock.hour(),|_,update,player,city| {
                                                let binding=event_cards::ai::Bindings {
                                                    world:event_cards::selection::World {
                                                        buildings:&city.buildings,businesses:&city.ledgers.businesses,categories:&city.categories,
                                                        catalog:&city.catalog.info,products:&city.catalog.products,commodities:&city.commodities.rows,
                                                        utilities:context.utilities.enabled.map(u8::from),hour:clock.hour() as i32,tick:clock.timing[1],
                                                        // Fresh-process BSS word7a3f90, outside the business pool.
                                                        unselected_building:0,
                                                    },
                                                    targets:event_cards::lifecycle::Targets {
                                                        buildings:&city.buildings,businesses:&city.ledgers.businesses,categories:&city.categories,
                                                        players:&players,nodes:&nodes,
                                                    },
                                                    period:clock.calendar[2],image,
                                                };
                                                    event_cards::ai::run_update(player,update,&binding,|call,player| {
                                                        match call {
                                                            event_cards::ai::RuntimeCall::Submit(command)=>{
                                                                if self.connected.is_some() {
                                                                    // Evaluation/reset still runs on every peer, including RNG draws.
                                                                    outgoing_cards.push(command);
                                                                    return Ok(());
                                                                }
                                                                if scheduler.network_enabled&&!scheduler.network_override {return Err("event command requires network transport".into());}
                                                                self.live_commands.submit(live_commands::Command::EventCard(command),clock.timing[0],true,
                                                                    |_|Err("wrapped event command requires immediate execution".into()))
                                                            },
                                                            event_cards::ai::RuntimeCall::Broadcast(args)=>{
                                                                // Original48cab0 admission and sender context;
                                                                // broadcast34 mutates the deck synchronously.
                                                                if !admitted {return Ok(());}
                                                                let packet=openmt2001::player_messages::descriptor(args,word(&player.bytes,12) as i32,policy.player_message_prior);
                                                                if scheduler.network_enabled&&!scheduler.network_override {
                                                                    boundary(Event::Command(live_commands::Event::PlayerMessageRoute {
                                                                        route:openmt2001::player_messages::Route::NetworkSend,packet,
                                                                    }),clock).map(|_|())
                                                                }else{self.event_cards.release(args[3] as u32)}
                                                            },
                                                            event_cards::ai::RuntimeCall::Diagnostic{kind,used}=>boundary(Event::Command(live_commands::Event::EventCardDiagnostic{kind,used}),clock).map(|_|()),
                                                        }
                                                    })
                                            })?;
                                            if let Some(connection) = &mut self.connected {
                                                for command in outgoing_cards {
                                                    let targets = openmt2001::network::commands::Targets::new(&city.buildings, &city.ledgers.businesses, &city.categories);
                                                    connection.commands.submit_ai_card(&connection.worker, command, &mut self.live_commands, &mut city.categories, targets, clock.timing[0])?;
                                                }
                                            }
                                        }
                                        CalendarEvent::Hourly(hourly::Event::Helper(0x4bb500)) => {
                                            let accounts = self.roster.account_prefixes(city)?;
                                            population.sample_history(
                                                &mut self.history,
                                                city,
                                                &accounts,
                                                image,
                                            )?;
                                        }
                                        CalendarEvent::Hourly(hourly::Event::Helper(0x44f2a0)) => {
                                            self.transport.advance_hour(
                                                &mut openmt2001::traffic::HourlyInputs {
                                                    era: clock.calendar[2],
                                                    hour: clock.hour(),
                                                    population: city.ledgers.residents.active_count
                                                        as i32,
                                                    tick: clock.timing[1],
                                                },
                                                &self.transport_models,
                                                &self.train_routes,
                                                policy.camera,
                                                |sound, _| {
                                                    boundary(Event::TransportSound(sound), clock)
                                                        .map(|_| ())
                                                },
                                            )?;
                                        }
                                        other => {
                                            return boundary(Event::Calendar(other), clock)
                                                .map(|v| v != 0);
                                        }
                                    }
                                    Ok(false)
                                },
                            ))?;
                        }
                        scheduler::Event::Helper(0x4805b0) => self.window_texture.tick(clock.hour()),
                        scheduler::Event::Helper(0x4b5d50) => self.pulse_lights.tick(),
                        scheduler::Event::Helper(0x44ec50) => {
                            self.transport.advance_logic(
                                image,
                                &self.transport_models,
                                clock.timing[1],
                                policy.camera,
                                |event| {
                                    boundary(
                                        match event {
                                            openmt2001::traffic::scene::LogicEvent::Environment {
                                                call, position,
                                            } => Event::TransportEnvironment { call, position },
                                            openmt2001::traffic::scene::LogicEvent::Sound(call) => {
                                                Event::TransportSound(call)
                                            }
                                        },
                                        clock,
                                    )
                                    .map(|_| ())
                                },
                            )?;
                        }
                        scheduler::Event::Helper(0x4d5b40) => {
                            script_context.update_clock(clock);
                            let players: Vec<_> = self
                                .roster
                                .manager
                                .players
                                .iter()
                                .map(|p| {
                                    Some(business_binding::Player {
                                        identity: p.reference,
                                        kind: u32::from_le_bytes(
                                            p.bytes[0x154..0x158].try_into().unwrap(),
                                        ),
                                    })
                                })
                                .collect();
                            std::mem::swap(context.utilities, &mut self.hooks.utilities);
                            let command_start = self.hooks.commands.len();
                            let result = if self.hooks.forced_auction_handlers.is_empty(){
                                self.scripts.tick_events(&mut city.script_state(clock, &players),&script_context,
                                    &mut geometry,&mut self.hooks)
                            }else{
                                self.scripts.tick_events_with_invocation(|vm,bindings,invocation,events|{
                                    let identity=vm.event_reference_identity(invocation.reference)?;
                                    let allowed=identity.is_some_and(|(name,body)|
                                        self.hooks.forced_auction_handlers.get(&name)==Some(&body));
                                    let first=self.hooks.commands.len();
                                    self.hooks.forced_auction_allowed=allowed;
                                    let result=vm.call_reference(invocation.reference,invocation.argument,&mut |id,args|
                                        bindings.handle(id,args,&mut city.script_state(clock, &players),events,
                                            &script_context,&mut geometry,&mut self.hooks));
                                    self.hooks.forced_auction_allowed=false;
                                    let result=result?;
                                    let mut commands=self.hooks.commands[first..].iter().filter_map(|(command,_)|
                                        if let openmt2001::level_script::Command::ForcedAuction{producer,participant,amount}=command{
                                            Some((*producer,*participant,*amount))
                                        }else{None});
                                    if let Some((producer,participant,amount))=commands.next(){
                                        if !allowed || commands.next().is_some(){return Err("forced auction is not one audited terminal call".into());}
                                        if city.auction.controller.reference==0{return Err("forced auction has no active controller".into());}
                                        if scheduler.network_enabled && !scheduler.network_override{return Err("forced auction network routing unsupported".into());}
                                        let action=openmt2001::ai_messages::offers::auction::forced_action_descriptor(
                                            producer,participant,amount,(&city.auction.controller.record[..128]).try_into().unwrap(),[0;32])?;
                                        let mut descriptor=[0;52];descriptor[..32].copy_from_slice(&action);
                                        let mut utilities=std::mem::replace(&mut self.hooks.utilities,
                                            openmt2001::economy::Utilities::new([0;4]));
                                        let applied={
                                        let hooks=std::cell::RefCell::new(&mut self.hooks);
                                        let mut bridge=super::tutorial_announcement::PartsBridge{
                                            runtime:vm,bindings,events,hooks:&hooks,context:&mut script_context};
                                        let mut messages=messages.borrow_mut();
                                        let mut runtime=live_commands::Runtime{city,roster:&mut self.roster,
                                            population:dispatch.population,utilities:&mut utilities,board,
                                            event_cards:&mut self.event_cards,message_context:&mut messages};
                                        runtime.auction_phase_with_tutorial(&descriptor,clock,
                                            [scheduler.network_enabled,scheduler.network_override],
                                            Some((self.tutorial_state,&mut geometry,&mut bridge)),
                                            |call,clock|boundary(Event::Command(call),clock))
                                        };
                                        self.hooks.utilities=utilities;
                                        applied?;
                                    }
                                    Ok(result)
                                })
                            };
                            std::mem::swap(context.utilities, &mut self.hooks.utilities);
                            script_context.update_clock(clock);
                            result?;
                            dispatch.population.refresh_bindings_if_changed(city,&self.resident_models,&self.people)?;
                            if self.hooks.commands.len() != command_start {
                                boundary(Event::ScriptCommands(&self.hooks.commands[command_start..]), clock)?;
                            }
                            dispatch.population.refresh_city(city, board)?;
                        }
                        scheduler::Event::Helper(0x465230)=>{
                            if self.connected.as_ref().is_some_and(|c|c.commands.auction_control_pending()){return Ok(0);}
                            let hooks=std::cell::RefCell::new(&mut self.hooks);
                            let mut announcement=super::tutorial_announcement::Bridge{
                                scripts:&mut self.scripts,hooks:&hooks,context:&mut script_context};
                            let mut runtime=live_commands::Runtime{city,roster:&mut self.roster,population:dispatch.population,
                                utilities:context.utilities,board,event_cards:&mut self.event_cards,message_context:&mut messages.borrow_mut()};
                            let mut outgoing=Vec::new();
                            runtime.advance_auction_with_tutorial(clock,[scheduler.network_enabled,scheduler.network_override],
                                Some((self.tutorial_state,&mut geometry,&mut announcement)),|event,clock|{
                                if let live_commands::Event::AuctionQueue{descriptor,additional_delay}=event{
                                    if self.connected.is_some(){outgoing.push((descriptor,additional_delay));Ok(0)}else{
                                        self.live_commands.submit_with_delay(live_commands::Command::Auction{descriptor},clock.timing[0],descriptor[16]!=0,additional_delay,
                                            |_|Err("auction update command wrapped into immediate dispatch".into())).map(|_|0)
                                    }
                                }else if let live_commands::Event::PlayerMessageRoute{route:openmt2001::player_messages::Route::NetworkRecipient{..},packet}=event
                                    && self.connected.is_some() && matches!(u32::from_le_bytes(packet[4..8].try_into().unwrap()),0x27..=0x2a){
                                    let mut descriptor=[0;52];descriptor[..48].copy_from_slice(&packet);outgoing.push((descriptor,0));Ok(0)
                                }else{boundary(Event::Command(event),clock)}
                            })?;
                            if let Some(connection)=&mut self.connected {
                                for (descriptor,additional) in outgoing {
                                    connection.commands.submit_auction(&connection.worker,descriptor,additional,&mut self.live_commands,&mut runtime.city.categories,clock.timing[0])?;
                                }
                            }
                        }
                        scheduler::Event::Helper(0x480ec0) => {
                            let mut clocks = clock.timing[..2].try_into().unwrap();
                            let mut runtime = live_commands::Runtime {
                                city,
                                roster: &mut self.roster,
                                population: dispatch.population,
                                utilities: context.utilities,
                                board,
                                event_cards: &mut self.event_cards,
                                message_context: &mut messages.borrow_mut(),
                            };
                            let mut outgoing_auctions=Vec::new();
                            let mut prepared_room=false;
                            self.live_commands.drain_scheduling(&mut clocks, |command,enqueue| {
                                if self.connected.is_some() && !matches!(command,live_commands::Command::Auction{..}|live_commands::Command::CategoryReady{..}|live_commands::Command::PropertyTransfer {..}|live_commands::Command::Build(_)|live_commands::Command::Demolish(_)|live_commands::Command::Open(_)|live_commands::Command::Convert(_)|live_commands::Command::SellBusiness(_)|live_commands::Command::Takeover(_)|live_commands::Command::Terms(_)|live_commands::Command::EventCard(_)) {
                                    return Err(openmt2001::network::Error::Unsupported.to_string());
                                }
                                if let live_commands::Command::Auction{descriptor}=command
                                    && matches!(u32::from_le_bytes(descriptor[4..8].try_into().unwrap()),0x1e..=0x20){
                                        let hooks=std::cell::RefCell::new(&mut self.hooks);
                                        let mut announcement=super::tutorial_announcement::Bridge{
                                            scripts:&mut self.scripts,hooks:&hooks,context:&mut script_context};
                                        return runtime.auction_phase_with_tutorial(descriptor,clock,[scheduler.network_enabled,scheduler.network_override],
                                            Some((self.tutorial_state,&mut geometry,&mut announcement)),|event,clock|{
                                            if let live_commands::Event::AuctionQueue{descriptor,additional_delay}=event {
                                                if self.connected.is_some(){outgoing_auctions.push((descriptor,additional_delay));Ok(0)}else{
                                                    enqueue(live_commands::Command::Auction{descriptor},openmt2001::local_commands::Schedule::Local{delayed:descriptor[16]!=0,additional:additional_delay}).map(|_|0)
                                                }
                                            }else{
                                                if let live_commands::Event::PlayerMessageRoute{route:openmt2001::player_messages::Route::NetworkRecipient{..},packet}=event
                                                    && self.connected.is_some() && matches!(u32::from_le_bytes(packet[4..8].try_into().unwrap()),0x27..=0x2a){
                                                    let mut descriptor=[0;52];descriptor[..48].copy_from_slice(&packet);outgoing_auctions.push((descriptor,0));return Ok(0);
                                                }
                                                if matches!(event,live_commands::Event::AuctionNetworkBarrier){prepared_room=true;return Ok(0);}
                                                if matches!(event,live_commands::Event::AuctionActivation(openmt2001::ai_messages::offers::auction::activation::Call::NormalSpeed)){scheduler.iterations=1;}
                                                boundary(Event::Command(event),clock)
                                            }
                                        });
                                }
                            let mut command_context = live_commands::CommandContext {
                                image,
                                clock: &mut *clock,
                                accounting_enabled: true,
                                notifications: runtime.city.ai_demand.ready != 0,
                                tutorial_state: self.tutorial_state,
                                network_enabled: scheduler.network_enabled,
                                network_override: scheduler.network_override,
                                utility_factor: runtime.city.settings.selected_income().multiplier,
                            };
                                let hooks=std::cell::RefCell::new(&mut self.hooks);
                                let mut announcement=super::tutorial_announcement::Bridge{
                                    scripts:&mut self.scripts,hooks:&hooks,context:&mut script_context};
                                let result = runtime.apply_with_announcement(command, &mut command_context, &mut geometry, Some(&mut announcement), |event,clock| {
                                    if let live_commands::Event::AuctionQueue{descriptor,additional_delay}=event {
                                        if self.connected.is_some(){outgoing_auctions.push((descriptor,additional_delay));Ok(())}else{
                                            enqueue(live_commands::Command::Auction{descriptor},openmt2001::local_commands::Schedule::Local{delayed:descriptor[16]!=0,additional:additional_delay})
                                        }
                                    }else if let live_commands::Event::CategoryReadyAt{category,normal_tick}=event{
                                        enqueue(live_commands::Command::CategoryReady{category},openmt2001::local_commands::Schedule::Normal{due:normal_tick})
                                    }else if let live_commands::Event::Outcome(openmt2001::outcome_notice::Call::Timers(timers))=event {
                                        timers.apply(&mut hooks.borrow_mut().timers);Ok(())
                                    }else if matches!(event,live_commands::Event::Elimination(openmt2001::player_elimination::Call::FinishLocal)) {
                                        openmt2001::player_elimination::finish_local(self.network_command_timer,&mut hooks.borrow_mut().timers)?;
                                        boundary(Event::Command(event),clock).map(|_|())
                                    }else{
                                        if matches!(event,live_commands::Event::Outcome(openmt2001::outcome_notice::Call::Show)) && !scheduler.network_enabled {scheduler.iterations=1;}
                                        boundary(Event::Command(event), clock).map(|_| ())
                                    }
                                });
                                announcement.context.update_clock(command_context.clock);
                                result
                            })?;
                            if let Some(connection)=&mut self.connected {
                                if prepared_room {
                                    connection.synchronization.auction_prepared(&mut runtime.city.pause,&mut self.hooks.timers)?;
                                }
                                for (descriptor,additional) in outgoing_auctions {
                                    connection.commands.submit_auction(&connection.worker,descriptor,additional,&mut self.live_commands,&mut runtime.city.categories,clock.timing[0])?;
                                }
                            }

                        }
                        scheduler::Event::Helper(0x4b0810) => {
                            let mut removed = false;
                            let tree_state = geometry.registrar().tree_state.clone();
                            let mut completions = Vec::new();
                            live_commands::PendingRuntime {
                                city,
                                roster: &self.roster,
                                commands: &mut self.live_commands,
                                selected: &mut self.building_selection,
                            }
                            .advance_with_submission(
                                clock.timing[0],
                                &mut openmt2001::pending_world::Controls {
                                    network_enabled: scheduler.network_enabled,
                                    network_override: self.connected.as_ref().map_or(scheduler.network_override, |c| c.commands.is_host()),
                                    effect_model_reference: policy.effect_model,
                                },
                                &mut geometry,
                                |event, mut record| {
                                    match event {
                                        live_commands::PendingCall::World(openmt2001::pending_world::Event::CreateEffect { arguments }) => {
                                            return self.demolition_effects.create(arguments);
                                        }
                                        live_commands::PendingCall::ReleaseEffect { index, .. } => {
                                            self.demolition_effects.release(index)?;
                                            return Ok(0);
                                        }
                                        _ => (),
                                    }
                                    if matches!(
                                        event,
                                        live_commands::PendingCall::ReleaseBuilding { .. }
                                    ) {
                                        crate::building_models::release_record_with_trees(
                                            record.as_deref_mut().ok_or("missing building release record")?, tree_state.as_ref(),
                                        )?;
                                        removed = true;
                                    }
                                    // The owned Models registry was released above;
                                    // no original renderer resource pointer escapes.
                                    if matches!(event, live_commands::PendingCall::ReleaseBuilding { .. }) { Ok(0) }
                                    else { boundary(Event::Pending(event, record), clock) }
                                },
                                |command, commands| {
                                    if self.connected.is_some() {
                                        completions.push(command);
                                        Ok(())
                                    } else {
                                        commands.submit(command, clock.timing[0], true,
                                            |_| Err("wrapped construction completion requires reentrant command dispatch".into()))
                                    }
                                },
                            )?;
                            if let Some(connection) = &mut self.connected {
                                for command in completions {
                                    let live_commands::Command::Open(command) = command else {
                                        return Err("pending completion did not produce a business open command".into());
                                    };
                                    let targets = openmt2001::network::commands::Targets::new(&city.buildings, &city.ledgers.businesses, &city.categories);
                                    connection.commands.submit_completion(&connection.worker, &command, &mut self.live_commands, &mut city.categories,
                                        targets, clock.timing[0])?;
                                }
                            }
                            if removed {
                                geometry
                                    .registrar_mut()
                                    .bindings
                                    .retain(|slot, _| city.buildings[*slot][0x3f] != 0);
                                dispatch.population.refresh_city(city, board)?;
                            }
                        }
                        scheduler::Event::Helper(0x477f20) => {
                            openmt2001::ai_events::poll_players(&mut self.roster.manager.players,&mut self.roster.profiles,clock.timing[3] as i32)?;
                        }
                        scheduler::Event::Helper(0x48cea0) => {
                            self.roster.publish_accounts(city)?;
                            let mut controls=openmt2001::player_tick::Controls {
                                network_enabled:scheduler.network_enabled,
                                network_override:scheduler.network_override,
                                current_player:self.roster.status.current_player,
                            };
                            if let Some(connection) = &self.connected { connection.commands.configure_players(&mut controls); }
                            let result=openmt2001::player_tick::advance(
                                &mut self.roster.manager,&mut controls,image,
                                |event,manager,controls|match event {
                                    openmt2001::player_tick::Event::Controller{player,target:0x48c140}=>{
                                        let actors:Vec<_>=manager.players.iter().map(|p|openmt2001::human_messages::Actor{reference:p.reference,character:u32::from_le_bytes(p.bytes[0x150..0x154].try_into().unwrap())}).collect();
                                        let names:Vec<(u32,[u8;96])>=manager.players.iter().map(|p|(p.reference+0x158,p.bytes[0x158..0x1b8].try_into().unwrap())).collect();
                                        let player=manager.players.iter_mut().find(|p|p.reference==player).ok_or("human player unresolved")?;
                                        openmt2001::player_tick::advance_human(player,|player|{
                                            let world=openmt2001::human_messages::World{image,actors:&actors,categories:&city.categories,buildings:&city.buildings,businesses:&city.ledgers.businesses,catalog:city.catalog.info.rows()};
                                            openmt2001::human_messages::consume(player,&world,|call,_|{
                                                // Normal prompt open429b50 calls481910(0). The
                                                // original speed setter changes only offline games.
                                                if matches!(call,openmt2001::human_messages::Call::Ui{address:0x43be20,..})&&!scheduler.network_enabled{scheduler.iterations=1;}
                                                let name=if let openmt2001::human_messages::Call::Ui{address:0x440d40,arguments,..}=call{
                                                    Some(names.iter().find(|(reference,_)|*reference==arguments[1]).ok_or("human format name pointer unresolved")?.1)
                                                }else{None};
                                                boundary(Event::HumanDialog{call,name},clock).map(|_|())
                                            })
                                        })
                                    }
                                    openmt2001::player_tick::Event::Controller{player,target:0x466ca0}=>{
                                        let slot=manager.players.iter().position(|p|p.reference==player).ok_or("normalAI player unresolved")?;
                                        // Auction actions below change controller/UI state only;
                                        // money transactions enter the ordered command queue.
                                        let time_slot=manager.players.iter().position(|p|p.reference==controls.current_player)
                                            .ok_or("AI auction Time current player missing")?;
                                        let time_cash=city.ledgers.players.get(time_slot)
                                            .ok_or("AI auction Time current account missing")?.cash;
                                        let mut message_context=messages.borrow_mut();
                                        let mut world=openmt2001::ai_runtime::World{
                                            building_parameters:city.building_parameters,
                                            neighbor_cache:&mut city.ai_neighbors,
                                            ownership_refresh_counter:&mut city.ownership_refresh_counter,
                                            utility_enabled:context.utilities.enabled,
                                            liquidation_targets:&mut self.roster.liquidation_targets,
                                            colors:Some(openmt2001::ai_messages::offers::execution::Colors{
                                                banks:[&self.building_styles.qualities[0],&self.building_styles.qualities[1],&self.building_styles.qualities[2]],
                                                players:&self.player_style_indices,
                                            }),
                                            category_counts:&mut self.roster.category_counts,
                                            ai_demand_rows:&city.ai_demand.rows,
                                    active_player_count:u32::from_le_bytes(manager.bytes[12..16].try_into().unwrap()),
                                            construction_components:&mut city.construction_components,
                                            construction_allowance:i32::from_le_bytes(image.at(0x541c7c,4)?.try_into().unwrap()),
                                            construction_discount:image.f32_at(0x53be78)?,
                                            affordability_override:false,
                                            acquisition_checks:false, //4b91b1 startup default.
                                            proposal_identity:&mut self.roster.proposal_identity,
                                            residents:&city.ledgers.residents,
                                            commodity_active:city.commodities.rows.map(|row|row[0x10]!=0),
                                            catalog:&city.catalog.info,
                                            products:&city.catalog.products,
                                            commodities:&city.commodities.rows,
                                            category_footprints:&mut city.category_footprints,
                                            strategy_time:Some(openmt2001::ai_selection::Time{hour:clock.hour() as i32,year:clock.calendar[2] as i32}),
                                            ownership_tick:Some(clock.timing[1]),
                                            strategy_mode:city.objective.kind,
                                            business_counts:Some(&mut dispatch.population.business_counts),
                                            message_routing:Some(openmt2001::player_messages::Routing {
                                                current_player:message_context.current_player,
                                                network:message_context.network,prior:message_context.prior,
                                            }),
                                            categories:&mut city.categories,buildings:&city.buildings,businesses:&mut city.ledgers.businesses,
                                            ownership:&mut self.roster.ai_ownership,
                                            allocation_next:&mut message_context.arena.next,
                                            global_reset:&mut city.ai_demand.ready,
                                            income:openmt2001::ai_finance::IncomeTotals::from_economy(&dispatch.population.properties,context.utilities),
                                            auction:city.auction.message_view(),auction_blocked:Some(city.auction.controller.reference!=0 && city.auction.controller.record[0x7d]!=0),auction_guard:city.auction.guard_view(),offer_auction:city.auction.offer_view(),
                                            auction_state:Some(&mut city.auction.controller),
                                            cash_statuses:&city.objective.cash_statuses,
                                        };
                                        openmt2001::ai_runtime::advance(slot,manager,&mut self.roster.profiles,
                                            &mut openmt2001::ai_tick::Controls{normal_tick:clock.timing[0],day_tick:clock.timing[3],event_gate:policy.ai_event_gate},
                                            &mut self.roster.scripts,image,&mut world,|call,player,profiles,world|{
                                                if let openmt2001::ai_runtime::Call::ConstructionCommand{descriptor}=call {
                                                    let command=openmt2001::construction::Command::from_packet(&descriptor)?;
                                                    if let Some(connection) = &mut self.connected {
                                                        let targets = openmt2001::network::commands::Targets::new(world.buildings, world.businesses, world.categories);
                                                        connection.commands.submit(&connection.worker,live_commands::Command::Build(command),
                                                            &mut self.live_commands,world.categories, targets,clock.timing[0])?;
                                                    } else {
                                                        self.live_commands.submit_with_delay(live_commands::Command::Build(command),clock.timing[0],descriptor[16]!=0,0,
                                                            |_|Err("AI construction delay wrapped into immediate execution".into()))?;
                                                    }
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                if let openmt2001::ai_runtime::Call::OfferExecution(openmt2001::ai_messages::offers::execution::Call::Producer{address:0x48ae60,arguments,count:1})=call {
                                                    let routing=world.message_routing.ok_or("AI sale command routing unresolved")?;
                                                    let command=live_commands::Command::SellBusiness(openmt2001::business_sale::Command{business:arguments[0],player:player.bytes[0x1bc]});
                                                    if let Some(connection) = &mut self.connected {
                                                        let targets = openmt2001::network::commands::Targets::new(world.buildings, world.businesses, world.categories);
                                                        connection.commands.submit(&connection.worker, command, &mut self.live_commands, world.categories, targets, clock.timing[0])?;
                                                    } else {
                                                        if routing.network[0]&&!routing.network[1]{return Err("AI sale command requires network transport".into());}
                                                        self.live_commands.submit_with_delay(command,clock.timing[0],true,0,|_|Err("AI sale delay wrapped into immediate execution".into()))?;
                                                    }
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                if let openmt2001::ai_runtime::Call::OfferExecution(openmt2001::ai_messages::offers::execution::Call::Producer{address:address @ (0x48ad20|0x48ad80),arguments,count:3})=call {
                                                    let routing=world.message_routing.ok_or("AI business command routing unresolved")?;
                                                    let command=if address==0x48ad20 {
                                                        live_commands::Command::Demolish(openmt2001::demolition::Command{category:arguments[0] as i8,building:arguments[1] as u16,amount:f32::from_bits(arguments[2])})
                                                    }else{
                                                        live_commands::Command::Open(openmt2001::business_open::Command::from_completion(player,arguments[0],arguments[1],arguments[2] as u8)?)
                                                    };
                                                    if let Some(connection) = &mut self.connected {
                                                        let targets = openmt2001::network::commands::Targets::new(world.buildings, world.businesses, world.categories);
                                                        connection.commands.submit(&connection.worker, command, &mut self.live_commands, world.categories,
                                                            targets, clock.timing[0])?;
                                                    } else {
                                                        if routing.network[0]&&!routing.network[1]{return Err("AI business command requires network transport".into());}
                                                        self.live_commands.submit_with_delay(command,clock.timing[0],true,0,|_|Err("AI business command delay wrapped into immediate execution".into()))?;
                                                    }
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                if let openmt2001::ai_runtime::Call::AuctionCommand{descriptor,additional_delay}=call {
                                                    if let Some(connection)=&mut self.connected {
                                                        connection.commands.submit_auction(&connection.worker,descriptor,additional_delay,&mut self.live_commands,world.categories,clock.timing[0])?;
                                                    }else{
                                                    self.live_commands.submit_with_delay(live_commands::Command::Auction{descriptor},clock.timing[0],descriptor[16]!=0,additional_delay,
                                                        |_|Err("AI auction command delay wrapped into immediate execution".into()))?;
                                                    }
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                if let openmt2001::ai_runtime::Call::Management(openmt2001::ai_dispatch::Event::Helper(0x4683f0)) = call {
                                                    openmt2001::ai_maintenance::late_pricing(player,world.businesses,|command,businesses|{
                                                        if let Some(connection) = &mut self.connected {
                                                            let targets = openmt2001::network::commands::Targets::new(world.buildings, businesses, world.categories);
                                                            connection.commands.submit(&connection.worker, live_commands::Command::Terms(command), &mut self.live_commands, world.categories, targets, clock.timing[0])
                                                        } else {
                                                            self.live_commands.submit(live_commands::Command::Terms(command),clock.timing[0],true,|_|Err("AI late pricing at wrapped clock requires synchronous housing context".into()))
                                                        }
                                                    })?;
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                if let openmt2001::ai_runtime::Call::OfferExecution(openmt2001::ai_messages::offers::execution::Call::Producer{address:0x48adf0,arguments,count:2})=call {
                                                    let routing=world.message_routing.ok_or("AI conversion command routing unresolved")?;
                                                    let command=live_commands::Command::Convert(openmt2001::business_conversion::Command{kind:arguments[0],building:arguments[1],owner:u32::from(player.bytes[0x1bc])+1});
                                                    if let Some(connection) = &mut self.connected {
                                                        let targets = openmt2001::network::commands::Targets::new(world.buildings, world.businesses, world.categories);
                                                        connection.commands.submit(&connection.worker, command, &mut self.live_commands, world.categories, targets, clock.timing[0])?;
                                                    } else {
                                                        if routing.network[0]&&!routing.network[1]{return Err("AI conversion command requires network transport".into());}
                                                        self.live_commands.submit_with_delay(command,clock.timing[0],true,0,|_|Err("AI conversion delay wrapped into immediate execution".into()))?;
                                                    }
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                if let openmt2001::ai_runtime::Call::TakeoverCommand{building}=call {
                                                    let routing=world.message_routing.ok_or("AI takeover command routing unresolved")?;
                                                    let command=live_commands::Command::Takeover(openmt2001::business_takeover::Command{building});
                                                    if let Some(connection) = &mut self.connected {
                                                        let targets = openmt2001::network::commands::Targets::new(world.buildings, world.businesses, world.categories);
                                                        connection.commands.submit(&connection.worker, command, &mut self.live_commands, world.categories, targets, clock.timing[0])?;
                                                    } else {
                                                        if routing.network[0]&&!routing.network[1]{return Err("AI takeover command requires network transport".into());}
                                                        self.live_commands.submit_with_delay(command,clock.timing[0],true,0,|_|Err("AI takeover delay wrapped into immediate execution".into()))?;
                                                    }
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                if let openmt2001::ai_runtime::Call::Management(event @ (openmt2001::ai_dispatch::Event::BusinessSettings | openmt2001::ai_dispatch::Event::RentSettings)) = call {
                                                    let ownership = world.ownership.state(player)?;
                                                    let buildings: Vec<[u8;64]> = world.buildings.iter().map(|r| r[..64].try_into().unwrap()).collect();
                                                    let settings = economy::BusinessDaySettings::from_catalog(&city.catalog.info);
                                                    let inputs = economy::BusinessDayInputs {
                                                        buildings:&buildings,
                                                        building_categories:&dispatch.population.shopping.building_categories,
                                                        category_owners:&world.categories.map(|r|r[5]),
                                                        catalogs:&dispatch.population.settings.catalogs,
                                                        unit_prices:&dispatch.population.settings.unit_costs,
                                                        settings:&settings,
                                                    };
                                                    let submit = if event == openmt2001::ai_dispatch::Event::BusinessSettings {
                                                        openmt2001::ai_business::adjust_owned(&ownership.lists[0],world.businesses,&inputs)?;
                                                        true
                                                    }else{
                                                        let profile=profiles.get(u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()) as usize).ok_or("AI rent profile missing")?;
                                                        let rent=openmt2001::ai_rent::Inputs::from_profile(&inputs,context.utilities,profile,u32::from_le_bytes(player.bytes[0x13f4..0x13f8].try_into().unwrap()) as i32)?;
                                                        openmt2001::ai_rent::adjust_owned(&ownership.lists[0],world.businesses,&rent)?
                                                    };
                                                    if submit {
                                                        economy::submit_business_terms(world.businesses,i32::from(player.bytes[0x1bc]),|command,businesses|{
                                                            if let Some(connection) = &mut self.connected {
                                                                let targets = openmt2001::network::commands::Targets::new(world.buildings, businesses, world.categories);
                                                                connection.commands.submit(&connection.worker, live_commands::Command::Terms(command), &mut self.live_commands, world.categories, targets, clock.timing[0])
                                                            } else {
                                                                self.live_commands.submit(live_commands::Command::Terms(command),clock.timing[0],true,|_|{
                                                                    Err("AI terms at wrapped immediate-admission clock require synchronous housing context".into())
                                                                })
                                                            }
                                                        })?;
                                                    }
                                                    return Ok(openmt2001::ai_messages::Reply::default());
                                                }
                                                let event=if let openmt2001::ai_runtime::Call::AuctionAction(action)=call {
                                                    Event::Command(live_commands::auction_action_event(action,time_cash))
                                                } else { Event::Ai(call) };
                                                boundary(event,clock).map(|value|openmt2001::ai_messages::Reply{value,offer:0})
                                            })
                                    }
                                    other=>boundary(Event::Player(other),clock).map(|_|()),
                                });
                            self.roster.status.current_player=controls.current_player;
                            self.roster.collect_accounts(city)?;
                            result?;
                        }
                        scheduler::Event::Helper(0x4896e0) => {
                            messages.borrow_mut().current_player = self.roster.status.current_player;
                            let mode = u32::from_le_bytes(
                                self.roster.manager.bytes[0xc..0x10].try_into().unwrap(),
                            );
                            self.roster.evaluate_objective(
                                city,
                                image,
                                0x591318,
                                &mut openmt2001::objective_tick::Controls {
                                    network_enabled: scheduler.network_enabled,
                                    network_override: scheduler.network_override,
                                    mode,
                                    year: clock.calendar[2],
                                    day: clock.calendar[1],
                                },
                                |event, _, manager, _| {
                                    live_commands::queue_outcome(event, &mut self.live_commands,
                                        clock.timing[0], manager, &mut messages.borrow_mut(),
                                        |call| boundary(Event::Command(call), clock).map(|_| ()))
                                },
                            )?;
                        }
                        other => return boundary(Event::Tick(other), clock),
                    }
                    Ok(0)
                },
            );
            context.hourly.before_six_sound = policy.scheduler.before_six_sound;
            policy.hourly = context.hourly;
            result
        })();
        if policy.scheduler.iterations == admitted_iterations {
            policy.scheduler.iterations = requested_iterations;
        }
        self.models = std::mem::take(geometry.registrar_mut());
        self.hooks.utilities = utilities;
        self.allocation_next = messages.into_inner().arena.next;
        result
    }
}
