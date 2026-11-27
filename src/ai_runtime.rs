//! Connect the original normal controller to retained financial and inbox state.
//! Unrecovered decision operations remain synchronous, explicit dependencies.
use crate::{
    ai_messages, ai_tick,
    pe::PeImage,
    player_manager::{Manager, Player, word},
};

pub struct World<'a> {
    pub building_parameters: crate::building_parameters::BuildingKinds,
    pub neighbor_cache: &'a mut crate::ai_neighbors::Cache,
    pub ownership_refresh_counter: &'a mut u8,
    pub utility_enabled: [bool; 4],
    pub liquidation_targets: &'a mut std::collections::BTreeMap<u32, [u8; 12]>,
    pub colors: Option<crate::ai_messages::offers::execution::Colors<'a>>,
    pub category_counts: &'a mut [u32; 52 * 45],
    pub ai_demand_rows: &'a [[u8; 264]; 33],
    pub active_player_count: u32,
    pub construction_components: &'a mut [f32; 4],
    pub construction_allowance: i32,
    pub construction_discount: f32,
    pub affordability_override: bool,
    /// Original8b23c4; initialized clear by4b9140 at4b91b1.
    pub acquisition_checks: bool,
    pub proposal_identity: &'a mut u32,
    pub residents: &'a crate::economy::ResidentAccounts,
    pub commodity_active: [bool; 32],
    pub catalog: &'a crate::business_pool::BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub category_footprints: &'a mut [[i32; 2]; 52],
    pub strategy_time: Option<crate::ai_selection::Time>,
    pub ownership_tick: Option<u32>,
    pub strategy_mode: i32,
    pub business_counts: Option<&'a mut crate::player_metrics::BusinessCounts>,
    pub categories: &'a mut [[u8; 0x17c]; 52],
    pub buildings: &'a [[u8; 0x394]],
    pub businesses: &'a mut [[u8; 0x220]],
    pub ownership: &'a mut crate::ai_ownership::Storage,
    pub allocation_next: &'a mut u32,
    pub message_routing: Option<crate::player_messages::Routing>,
    pub global_reset: &'a mut u8,
    pub income: crate::ai_finance::IncomeTotals,
    pub auction: Option<ai_messages::Auction>,
    pub auction_blocked: Option<bool>,
    pub auction_guard: Option<crate::ai_guards::Auction>,
    pub offer_auction: Option<ai_messages::offers::Auction>,
    pub auction_state: Option<&'a mut ai_messages::offers::auction::admission::Controller>,
    pub cash_statuses: &'a [u8; 6],
}
#[derive(Clone, Copy, Debug)]
pub enum Call {
    Decision(ai_tick::Call),
    Message(ai_messages::Call),
    OfferAdmission(ai_messages::offers::Call),
    OfferExecution(ai_messages::offers::execution::Call),
    MessageRoute {
        route: crate::player_messages::Route,
        packet: crate::player_messages::Packet,
    },
    ///Already accepted/reserved by451720 admission; the host queues without re-admitting.
    ConstructionCommand {
        descriptor: [u8; 60],
    },
    TakeoverCommand {
        building: u32,
    },
    AuctionCommand {
        descriptor: [u8; 52],
        additional_delay: u32,
    },
    AuctionAction(ai_messages::offers::auction::Call),
    TimetableDiagnostic {
        secondary: bool,
        code: i32,
    },
    Management(crate::ai_dispatch::Event),
    Strategy(crate::ai_strategy::Call),
    Continuation(crate::ai_strategy::ContinuationCall),
    ContinuationQuery(crate::ai_actions::continuation::Call),
    Selection(crate::ai_selection::Call),
    Target(crate::ai_targets::Call),
    Candidate(crate::ai_candidate_fill::Call),
}

/// Original47c630, with caller-owned status/cost words retained across queries.
#[allow(clippy::too_many_arguments)]
pub fn planning(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    input: crate::ai_actions::Input,
    outputs: &mut [u32; 2],
    peers: &[Option<&Player>],
    world: &mut World<'_>,
    diagnostic: impl FnMut(
        crate::ai_actions::Call,
        &mut Player,
        &mut [[u8; 0x650]],
    ) -> Result<(), String>,
) -> Result<bool, String> {
    let references: Vec<_> = peers
        .iter()
        .map(|p| p.unwrap_or(player).reference)
        .collect();
    let mut state = crate::ai_actions::Planning {
        outputs: *outputs,
        ..Default::default()
    };
    let mut context = crate::ai_actions::retained::World {
        building_parameters: world.building_parameters,
        categories: world.categories,
        buildings: world.buildings,
        catalog: world.catalog,
        products: world.products,
        commodities: world.commodities,
        residents: world.residents,
        player_references: &references,
        player_count: world.active_player_count as i32,
        category_counts: world.category_counts,
        proposal_identity: world.proposal_identity,
        construction_components: world.construction_components,
        hour: world
            .strategy_time
            .ok_or("AI planning clock unresolved")?
            .hour,
        mode: world.strategy_mode as u32,
        allowance: world.construction_allowance,
        affordability_override: world.affordability_override,
        discount: world.construction_discount,
        // Explicit host policy for original uninitialized proposal stack padding.
        proposal_prior: [0; 104],
    };
    let result = crate::ai_actions::retained::choose(
        player,
        profiles,
        image,
        input,
        &mut state,
        &mut context,
        diagnostic,
    );
    *outputs = state.outputs;
    result
}

fn initial_target(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    world: &mut World<'_>,
    prior: u32,
) -> Result<crate::ai_selection::Reply, String> {
    let time = world.strategy_time.ok_or("AI candidate clock unresolved")?;

    let categories = &*world.categories;
    let buildings = world.buildings;
    let resident_kinds: Vec<_> = world
        .residents
        .nodes
        .get(..world.residents.active_count as usize)
        .ok_or("AI candidate resident count exceeds pool")?
        .iter()
        .map(|r| word(r.as_bytes(), 0x3c))
        .collect();
    // These candidate helpers never write the profiles. Keep
    // their read view separate from the outer callback borrow.
    let profile_view = profiles.to_vec();
    let fill_world = crate::ai_candidate_fill::World {
        image,
        profiles: &profile_view,
        categories,
        buildings,
        resident_kinds: &resident_kinds,
        time,
    };
    let mut kind = prior;
    let value = crate::ai_targets::initial(
        player,
        categories,
        buildings,
        image,
        &mut kind,
        |address, player, rows, count| {
            crate::ai_candidate_fill::fill(
                address == 0x472b90,
                player,
                &fill_world,
                rows,
                count,
                |call, player| {
                    use crate::ai_candidate_queries as q;
                    let id = call.arguments[0];
                    Ok(match call.address {
                        0x46c900 | 0x46c9f0 => u32::from(q::window_gate(
                            call.address == 0x46c900,
                            &categories[id as usize],
                            buildings,
                            world.businesses,
                            world.catalog,
                            image,
                        )?),
                        0x473ec0 => q::vacant_perimeter(&categories[id as usize]) as u32,
                        0x46e540 => {
                            q::owned_siblings(id, call.arguments[1], categories, buildings, image)?
                                as u32
                        }
                        0x47d110 => crate::ai_profiles::preferred_tier(id) as u32,
                        0x48b030 => {
                            crate::consumer_poll::start(
                                player,
                                id as i32,
                                categories,
                                buildings,
                                world.businesses,
                                world.residents,
                                &world.commodity_active,
                            )?;
                            0
                        }
                        0x48b130 => {
                            crate::consumer_poll::stop(player);
                            0
                        }
                        0x48b140 => {
                            crate::consumer_poll::clear(player);
                            0
                        }
                        0x473230 | 0x473420 => u32::from(q::feasible(
                            call.address == 0x473420,
                            crate::ai_selection::windows(player, profiles, time.hour)?,
                            &categories[id as usize],
                            buildings,
                            world.businesses,
                            world.catalog,
                        )?),
                        0x473740 | 0x472ed0 | 0x473030 => {
                            let scores = crate::ai_candidate_scores::World {
                                catalog: world.catalog,
                                products: world.products,
                                commodities: world.commodities,
                                category: &categories[call.arguments[1] as usize],
                                buildings,
                                businesses: world.businesses,
                                image,
                            };
                            let window = crate::ai_selection::windows(player, profiles, time.hour)?;
                            if call.address == 0x473740 {
                                crate::ai_candidate_scores::coverage(window, &scores)? as u32
                            } else {
                                crate::ai_candidate_scores::ownership(
                                    call.address == 0x472ed0,
                                    player,
                                    window,
                                    &scores,
                                )? as u32
                            }
                        }
                        _ => return Err(format!("unresolved candidate helper {:x}", call.address)),
                    })
                },
            )
        },
    )?;
    Ok(crate::ai_selection::Reply {
        value: value as u32,
        output: kind,
    })
}
fn neighbor_target(
    player: &mut Player,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
    world: &World<'_>,
    mode: &mut u32,
) -> Result<bool, String> {
    let context = crate::ai_neighbors::World {
        image,
        profiles,
        hour: world
            .strategy_time
            .ok_or("AI neighbor clock unresolved")?
            .hour,
        categories: world.categories,
        buildings: world.buildings,
        businesses: world.businesses,
        catalog: world.catalog,
        products: world.products,
        commodities: world.commodities,
        residents: world.residents,
        commodity_active: &world.commodity_active,
        footprints: world.category_footprints,
        cache: world.neighbor_cache,
    };
    crate::ai_neighbors::select(player, mode, &context)
}

fn finance(
    player: &mut Player,
    peers: &[Option<&Player>],
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
    world: &World<'_>,
    normal_tick: u32,
    periodic: bool,
) -> Result<(), String> {
    let due = u16::from_le_bytes(player.bytes[0xb4e..0xb50].try_into().unwrap());
    if periodic && normal_tick < u32::from(due) {
        return Ok(());
    }
    let count = usize::from(u16::from_le_bytes(
        player.bytes[0xb44..0xb46].try_into().unwrap(),
    ));
    let mut balances = Vec::with_capacity(count);
    if count != 0 {
        let rows = world
            .ownership
            .lists
            .get(&word(&player.bytes, 0xb3c))
            .ok_or("AI finance ownership list unresolved")?;
        for row in rows
            .get(..count)
            .ok_or("AI finance count exceeds ownership list")?
        {
            let slot = word(row, 4);
            // Native 466fa0 reads the first b44 physical rows even after
            // 46ae20 leaves a cleared -1 row among them. Index -1 reads
            // 0x7a3fa8, the zero-initialized word before the business pool
            // in this v1.4 fresh process (native_ai_finance_list_edges.py).
            let balance = if slot == u32::MAX {
                0.0
            } else {
                let business = world
                    .businesses
                    .get(slot as usize)
                    .ok_or("AI finance business leaves pool")?;
                f32::from_bits(word(business, 0x20))
            };
            balances.push(balance);
        }
    }
    let projected = crate::ai_finance::projected_income(
        player.bytes[0x1bc],
        world.categories,
        &world.income.railroads,
        &world.income.utilities,
        image,
    )?;
    crate::ai_finance::update_selected(
        player,
        peers,
        profiles,
        normal_tick,
        periodic,
        &balances,
        projected,
    )
}

fn score_query(
    call: crate::ai_scoring::Query,
    player: &Player,
    peers: &[Option<&Player>],
    image: &PeImage<'_>,
    world: &mut World<'_>,
) -> Result<crate::ai_scoring::Reply, String> {
    use crate::{
        ai_scoring::{Query, Reply},
        ai_strategy_queries as q,
    };
    let value = match call {
        Query::FirstDay => u32::from(
            world
                .strategy_time
                .ok_or("AI strategy clock unresolved")?
                .first_day(),
        ),
        Query::GroupSize(index) => q::group_size(&q::group(image, index)?) as u32,
        Query::OwnedInGroup(index) => q::owned_in_group(player, &q::group(image, index)?, |id| {
            q::category_owner(id, world.categories, world.buildings)
        })? as u32,
        Query::UtilityOrTransport(category) => u32::from(q::utility_or_transport(image, category)?),
        Query::Income => {
            return Ok(Reply {
                value: 0,
                extended: crate::ai_finance::income_extended(player, false)?,
            });
        }
        Query::OccupiedArea(category) => q::occupied_area(
            world
                .categories
                .get(category as usize)
                .ok_or("AI score area category missing")?,
            world.buildings,
        )? as u32,
        Query::OwnsGroup(index) => u32::from(q::owns_group(
            player.reference,
            &q::group(image, index)?,
            |id| {
                let owner = q::category_owner(id, world.categories, world.buildings)? as u8;
                if owner == 0 {
                    return Ok(0);
                }
                Ok(peers
                    .get(usize::from(owner) - 1)
                    .ok_or("AI group owner leaves roster")?
                    .unwrap_or(player)
                    .reference)
            },
        )?),
        Query::LandscapeCount(index) => q::landscape_count(
            &q::group(image, index)?,
            world.categories,
            world.buildings,
            world.businesses,
        )? as u32,
        Query::OwnedCategories => q::owned_categories(player)? as u32,
        Query::BusinessCount(slot) => world
            .business_counts
            .as_mut()
            .ok_or("AI business-count cache unresolved")?
            .get(slot as usize, peers.len(), world.businesses)?
            as u32,
    };
    Ok(value.into())
}

struct OfferPeers<'a> {
    before: &'a mut [Player],
    after: &'a mut [Player],
}
fn offer_predecision(
    player: &mut Player,
    arguments: [u32; 2],
    peers: &mut OfferPeers<'_>,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    world: &mut World<'_>,
    callback: &mut impl FnMut(
        Call,
        &mut Player,
        &mut [[u8; 0x650]],
        &mut World<'_>,
    ) -> Result<ai_messages::Reply, String>,
) -> Result<(), String> {
    use ai_messages::offers::{self, auction::valuation, negotiation};
    if world.auction.is_some() && world.offer_auction.is_none() {
        return Err("AI offer auction context unresolved".into());
    }
    offers::predecision(
        player,
        arguments[0],
        arguments[1],
        world.offer_auction,
        world.cash_statuses,
        image,
        |call, player| {
            match call {
                offers::Call::AuctionBudget => {
                    let scoring = valuation::scoring::World {
                        image,
                        categories: world.categories,
                        buildings: world.buildings,
                        businesses: world.businesses,
                        catalog: world.catalog,
                        railroad_income: &world.income.railroads,
                        loan_status: world.cash_statuses,
                        hour: world
                            .strategy_time
                            .ok_or("auction valuation clock unresolved")?
                            .hour,
                        residents: world.residents,
                        commodity_active: &world.commodity_active,
                        before: peers.before,
                        after: peers.after,
                    };
                    let mut amount = 0;
                    let extended = valuation::evaluate(
                        player,
                        1,
                        world.offer_auction.map(|a| a.category),
                        image.f32_at(0x5250e0)?,
                        &mut amount,
                        |call, player| scoring.dispatch(call, player),
                    )?;
                    return Ok(offers::Reply {
                        amount,
                        extended,
                        value: 0,
                    });
                }
                offers::Call::CategoryValue(category) => {
                    let category = world
                        .categories
                        .get(category as usize)
                        .ok_or("offer value category unresolved")?;
                    return Ok(offers::Reply {
                        extended: offers::execution::category_value(category, image)?,
                        ..Default::default()
                    });
                }
                offers::Call::Eligible { target, amount } => {
                    return Ok(offers::Reply {
                        value: u32::from(negotiation::eligible(player, target, amount)?),
                        ..Default::default()
                    });
                }
                offers::Call::Complete {
                    category,
                    amount,
                    recipient,
                } => negotiation::remember(player, category, amount, recipient, profiles)?,
                offers::Call::Submit(arguments) => {
                    let routing = world
                        .message_routing
                        .ok_or("offer request command routing unresolved")?;
                    if routing.network[0] && !routing.network[1] {
                        return Err("offer request requires network command transport".into());
                    }
                    callback(
                        Call::AuctionCommand {
                            descriptor: negotiation::request(arguments, [0; 52]),
                            additional_delay: 0,
                        },
                        player,
                        profiles,
                        world,
                    )?;
                }
                offers::Call::Reply { packet, kind } => {
                    let source = offers::packet(player, packet)?;
                    let routing = world
                        .message_routing
                        .ok_or("offer reply routing unresolved")?;
                    let mut arena = crate::player_messages::Arena {
                        next: *world.allocation_next,
                    };
                    let result = crate::player_messages::reply_with_players(
                        &source,
                        kind,
                        [0x5f5e0ff; 4],
                        &mut crate::player_messages::SplitPlayers {
                            before: peers.before,
                            current: player,
                            after: peers.after,
                        },
                        &mut arena,
                        routing,
                        |route, packet, players| {
                            if let crate::player_messages::Route::PropertyRefresh { category } =
                                route
                            {
                                crate::category_ownership::property_view(
                                    world
                                        .categories
                                        .get_mut(category as usize)
                                        .ok_or("refusal property category leaves pool")?,
                                    0,
                                    1,
                                );
                            }
                            callback(
                                Call::MessageRoute {
                                    route,
                                    packet: *packet,
                                },
                                players.current,
                                profiles,
                                world,
                            )
                            .map(|_| ())
                        },
                    );
                    *world.allocation_next = arena.next;
                    result?;
                }
                other => {
                    let result = callback(Call::OfferAdmission(other), player, profiles, world)?;
                    return Ok(offers::Reply {
                        value: result.value,
                        amount: result.value as i32,
                        ..Default::default()
                    });
                }
            }
            Ok(Default::default())
        },
    )
}

pub(crate) fn allocate(next: &mut u32, size: usize) -> Result<u32, String> {
    let reference = *next;
    *next = next
        .checked_add(
            u32::try_from(size)
                .map_err(|_| "AI allocation size overflow")?
                .checked_add(15)
                .ok_or("AI allocation alignment overflow")?
                & !15,
        )
        .ok_or("AI allocation arena exhausted")?;
    if reference == 0 {
        return Err("AI allocation begins at null".into());
    }
    Ok(reference)
}

fn expand_housing(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    peers: &[Option<&Player>],
    world: &mut World<'_>,
) -> Result<(), String> {
    let references: Vec<_> = peers
        .iter()
        .map(|p| p.unwrap_or(player).reference)
        .collect();
    let mut context = crate::ai_actions::retained::World {
        building_parameters: world.building_parameters,
        categories: world.categories,
        buildings: world.buildings,
        catalog: world.catalog,
        products: world.products,
        commodities: world.commodities,
        residents: world.residents,
        player_references: &references,
        player_count: world.active_player_count as i32,
        category_counts: world.category_counts,
        proposal_identity: world.proposal_identity,
        construction_components: world.construction_components,
        hour: world
            .strategy_time
            .ok_or("AI expansion clock unresolved")?
            .hour,
        mode: world.strategy_mode as u32,
        allowance: world.construction_allowance,
        affordability_override: world.affordability_override,
        discount: world.construction_discount,
        proposal_prior: [0; 104],
    };
    crate::ai_actions::expansion::build(
        player,
        profiles,
        world.category_footprints,
        world.cash_statuses,
        image,
        &mut context,
        |player| {
            let players: Vec<_> = peers.iter().map(|p| p.unwrap_or(player)).collect();
            world.neighbor_cache.groups(
                world.ownership_refresh_counter,
                player.bytes[0x1bc] as usize,
                &players,
                world.categories,
                world.residents,
                image,
            )
        },
        planning_diagnostic,
    )
}

fn liquidation_finalize(
    player: &mut Player,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
    world: &mut World<'_>,
) -> Result<(), String> {
    let context = crate::ai_actions::liquidation_score::World {
        mode: world.strategy_mode,
        cash_statuses: world.cash_statuses,
        auction_active: world.auction_guard.is_some_and(|a| a.active),
        developed: crate::ai_developed_targets::World {
            image,
            profiles,
            hour: world
                .strategy_time
                .ok_or("AI liquidation clock unresolved")?
                .hour,
            categories: world.categories,
            buildings: world.buildings,
            businesses: world.businesses,
            catalog: world.catalog,
            ownership: world.ownership,
            residents: world.residents,
            commodity_active: &world.commodity_active,
        },
    };
    // Original stack padding has no producer; zero prior is explicit host policy.
    crate::ai_actions::liquidation_producer::produce(
        player,
        world.liquidation_targets,
        [0; 12],
        &context,
        |call, _, _| match call {
            crate::ai_actions::liquidation_pool::Call::Allocate(size) => {
                allocate(world.allocation_next, size)
            }
            crate::ai_actions::liquidation_pool::Call::Release(_) => Ok(0),
        },
    )
}

fn ownership(
    player: &mut Player,
    image: &PeImage<'_>,
    world: &mut World<'_>,
) -> Result<(), String> {
    crate::ai_ownership::maintain_player(
        world.ownership,
        player,
        world.categories,
        world.buildings,
        world.businesses,
        image,
        world.catalog,
        |size| allocate(world.allocation_next, size),
    )
}

fn planning_diagnostic(
    call: crate::ai_actions::Call,
    _player: &mut Player,
    _profiles: &mut [[u8; 0x650]],
) -> Result<(), String> {
    // Native sprintf/4e6960 only report a debug warning and then continue.
    eprintln!(
        "Original AI planning diagnostic {:08x}: {:?}",
        call.address,
        &call.arguments[..call.count]
    );
    Ok(())
}

fn transaction_proposal(
    address: u32,
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    args: [u32; 2],
    image: &PeImage<'_>,
    world: &mut World<'_>,
) -> Result<bool, String> {
    crate::ai_actions::proposals::transaction(
        address,
        player,
        args,
        [0; 104],
        world.proposal_identity,
        |call, player, _| {
            use crate::ai_actions::proposals::Call as P;
            match call {
                P::CategoryValue(id) => ai_messages::offers::execution::category_value(
                    world
                        .categories
                        .get(id as usize)
                        .ok_or("AI proposal category value missing")?,
                    image,
                ),
                P::CategoryActive(id) => Ok(crate::x87::Sum::from_i32(i32::from(
                    world
                        .categories
                        .get(id as usize)
                        .ok_or("AI proposal category activity missing")?[0x74],
                ))),
                P::Diagnostic(address) => {
                    planning_diagnostic(
                        crate::ai_actions::Call {
                            address: 0x4e6960,
                            arguments: [1, 0, address, 0, 0, 0, 0, 0, 0, 0],
                            count: 3,
                        },
                        player,
                        profiles,
                    )?;
                    Ok(Default::default())
                }
                P::UnknownKind(kind) => {
                    Err(format!("AI transaction proposal unexpected kind{kind}"))
                }
            }
        },
    )
}

fn business_kind(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    peers: &[Option<&Player>],
    world: &mut World<'_>,
) -> Result<bool, String> {
    use crate::ai_business_choice::Call as Choice;
    let time = world
        .strategy_time
        .ok_or("AI business selection clock unresolved")?;
    // Execute=0 planning never inserts proposals or changes category counts;
    // retain this read view while the planner borrows its execution context.
    let counts = *world.category_counts;
    let profile_view = profiles.to_vec();
    let references: Vec<_> = peers
        .iter()
        .map(|p| p.unwrap_or(player).reference)
        .collect();
    let ranking = crate::ai_business_ranking::World {
        image,
        catalog: world.catalog,
        products: world.products,
        commodities: world.commodities,
        categories: world.categories,
        buildings: world.buildings,
        businesses: world.businesses,
        counts: &counts,
        demand: world.ai_demand_rows,
        residents: world.residents,
        time,
    };
    let mut context = crate::ai_actions::retained::World {
        building_parameters: world.building_parameters,
        categories: world.categories,
        buildings: world.buildings,
        catalog: world.catalog,
        products: world.products,
        commodities: world.commodities,
        residents: world.residents,
        player_references: &references,
        player_count: world.active_player_count as i32,
        category_counts: world.category_counts,
        proposal_identity: world.proposal_identity,
        construction_components: world.construction_components,
        hour: time.hour,
        mode: world.strategy_mode as u32,
        allowance: world.construction_allowance,
        affordability_override: world.affordability_override,
        discount: world.construction_discount,
        proposal_prior: [0; 104],
    };
    let mut probe = |player: &mut Player, outputs: &mut [u32; 2]| {
        let mut state = crate::ai_actions::Planning {
            outputs: *outputs,
            ..Default::default()
        };
        let result = crate::ai_actions::retained::choose(
            player,
            profiles,
            image,
            crate::ai_actions::Input {
                execute: 0,
                style_override: 0,
                style: 0,
            },
            &mut state,
            &mut context,
            planning_diagnostic,
        );
        *outputs = state.outputs;
        result
    };
    crate::ai_business_choice::select(
        player,
        &profile_view,
        world.categories,
        time,
        |call, player, outputs| match call {
            Choice::Plan => probe(player, outputs).map(u32::from),
            Choice::HousingEligible => {
                crate::ai_business_ranking::housing_eligible(player, &ranking).map(u32::from)
            }
            Choice::Rank(mode) => {
                crate::ai_business_ranking::rank(player, &profile_view, mode, &ranking, &mut probe)
                    .map(|v| v as u32)
            }
            Choice::Diagnostic => {
                eprintln!("Original AI business choice reported unexpected landscape kind");
                Ok(0)
            }
        },
    )
}

fn maintain_businesses(
    selling: bool,
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    peers: &[Option<&Player>],
    world: &mut World<'_>,
) -> Result<(), String> {
    let time = world
        .strategy_time
        .ok_or("AI maintenance clock unresolved")?;
    let profile_view = profiles.to_vec();
    let references: Vec<_> = peers
        .iter()
        .map(|p| p.unwrap_or(player).reference)
        .collect();
    let count = usize::from(u16::from_le_bytes(
        player.bytes[0xb46..0xb48].try_into().unwrap(),
    ));
    let entries = if count == 0 {
        &[][..]
    } else {
        world
            .ownership
            .lists
            .get(&word(&player.bytes, 0xb3c))
            .and_then(|r| r.get(..count))
            .ok_or("AI maintenance ownership allocation missing")?
    };
    let mut maintenance = crate::ai_maintenance::World {
        building_parameters: world.building_parameters,
        image,
        profiles: &profile_view,
        categories: world.categories,
        buildings: world.buildings,
        businesses: world.businesses,
        catalog: world.catalog,
        entries,
        counts: world.category_counts,
        identity: world.proposal_identity,
        prior: [0; 104],
    };
    crate::ai_maintenance::maintain(selling, player, &mut maintenance, |player, mode, w| {
        let counts = *w.counts;
        let ranking = crate::ai_business_ranking::World {
            image,
            catalog: w.catalog,
            products: world.products,
            commodities: world.commodities,
            categories: w.categories,
            buildings: w.buildings,
            businesses: w.businesses,
            counts: &counts,
            demand: world.ai_demand_rows,
            residents: world.residents,
            time,
        };
        let mut context = crate::ai_actions::retained::World {
            building_parameters: world.building_parameters,
            categories: w.categories,
            buildings: w.buildings,
            catalog: w.catalog,
            products: world.products,
            commodities: world.commodities,
            residents: world.residents,
            player_references: &references,
            player_count: world.active_player_count as i32,
            category_counts: w.counts,
            proposal_identity: w.identity,
            construction_components: world.construction_components,
            hour: time.hour,
            mode: world.strategy_mode as u32,
            allowance: world.construction_allowance,
            affordability_override: world.affordability_override,
            discount: world.construction_discount,
            proposal_prior: [0; 104],
        };
        crate::ai_business_ranking::rank(
            player,
            &profile_view,
            mode,
            &ranking,
            |player, outputs| {
                let mut state = crate::ai_actions::Planning {
                    outputs: *outputs,
                    ..Default::default()
                };
                let result = crate::ai_actions::retained::choose(
                    player,
                    profiles,
                    image,
                    crate::ai_actions::Input {
                        execute: 0,
                        style_override: 0,
                        style: 0,
                    },
                    &mut state,
                    &mut context,
                    planning_diagnostic,
                );
                *outputs = state.outputs;
                result
            },
        )
    })
}

fn portfolio_value(
    player: &Player,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
    world: &mut World<'_>,
) -> Result<i32, String> {
    let count = usize::from(u16::from_le_bytes(
        player.bytes[0xb44..0xb46].try_into().unwrap(),
    ));
    let entries = if count == 0 {
        &[][..]
    } else {
        world
            .ownership
            .lists
            .get(&word(&player.bytes, 0xb3c))
            .and_then(|r| r.get(..count))
            .ok_or("AI portfolio ownership allocation missing")?
    };
    crate::ai_maintenance::portfolio(
        player,
        &crate::ai_maintenance::World {
            building_parameters: world.building_parameters,
            image,
            profiles,
            categories: world.categories,
            buildings: world.buildings,
            businesses: world.businesses,
            catalog: world.catalog,
            entries,
            counts: world.category_counts,
            identity: world.proposal_identity,
            prior: [0; 104],
        },
    )
}

fn developed_target(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    peers: &[Option<&Player>],
    world: &mut World<'_>,
) -> Result<bool, String> {
    let time = world
        .strategy_time
        .ok_or("AI developed-target clock unresolved")?;
    let profile_view = profiles.to_vec();
    let references: Vec<_> = peers
        .iter()
        .map(|p| p.unwrap_or(player).reference)
        .collect();
    let target = crate::ai_developed_targets::World {
        image,
        profiles: &profile_view,
        hour: time.hour,
        categories: world.categories,
        buildings: world.buildings,
        businesses: world.businesses,
        catalog: world.catalog,
        ownership: world.ownership,
        residents: world.residents,
        commodity_active: &world.commodity_active,
    };
    let mut context = crate::ai_actions::retained::World {
        building_parameters: world.building_parameters,
        categories: world.categories,
        buildings: world.buildings,
        catalog: world.catalog,
        products: world.products,
        commodities: world.commodities,
        residents: world.residents,
        player_references: &references,
        player_count: world.active_player_count as i32,
        category_counts: world.category_counts,
        proposal_identity: world.proposal_identity,
        construction_components: world.construction_components,
        hour: time.hour,
        mode: world.strategy_mode as u32,
        allowance: world.construction_allowance,
        affordability_override: world.affordability_override,
        discount: world.construction_discount,
        proposal_prior: [0; 104],
    };
    crate::ai_developed_targets::select(player, &target, |player, outputs| {
        let mut state = crate::ai_actions::Planning {
            outputs: *outputs,
            ..Default::default()
        };
        let result = crate::ai_actions::retained::choose(
            player,
            profiles,
            image,
            crate::ai_actions::Input {
                execute: 0,
                style_override: 0,
                style: 0,
            },
            &mut state,
            &mut context,
            planning_diagnostic,
        );
        *outputs = state.outputs;
        result
    })
}

fn dispatch(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    before: &[Player],
    after: &[Player],
    world: &mut World<'_>,
    callback: &mut impl FnMut(
        Call,
        &mut Player,
        &mut [[u8; 0x650]],
        &mut World<'_>,
    ) -> Result<ai_messages::Reply, String>,
) -> Result<(), String> {
    let peer_rows: Vec<_> = before
        .iter()
        .map(Some)
        .chain(std::iter::once(None))
        .chain(after.iter().map(Some))
        .collect();
    let peers = peer_rows.as_slice();
    let mut flags = word(&player.bytes, 0x220);
    let mut blocked = if flags & 0x40 != 0 {
        world.auction_blocked.ok_or("AI auction gate unresolved")?
    } else {
        false
    };
    let result = crate::ai_dispatch::advance(
        &mut flags,
        &mut blocked,
        |event, flags, blocked, choices| {
            crate::player_manager::put(&mut player.bytes, 0x220, *flags);
            let result = if event == crate::ai_dispatch::Event::Ownership {
                ownership(player, image, world).map(|_| 0)
            } else if event == crate::ai_dispatch::Event::Helper(0x468920) {
                expand_housing(player, profiles, image, peers, world).map(|_| 0)
            } else if event == crate::ai_dispatch::Event::Helper(0x47c4f0) {
                let inputs = ai_messages::offers::auction::valuation::scoring::World {
                    image,
                    categories: world.categories,
                    buildings: world.buildings,
                    businesses: world.businesses,
                    catalog: world.catalog,
                    railroad_income: &world.income.railroads,
                    loan_status: world.cash_statuses,
                    hour: world
                        .strategy_time
                        .ok_or("AI auction seeking calendar unresolved")?
                        .hour,
                    residents: world.residents,
                    commodity_active: &world.commodity_active,
                    before,
                    after,
                };
                let auction = world
                    .offer_auction
                    .map(|a| (a.category, world.auction_blocked.unwrap_or(false)));
                crate::ai_acquisition::seek_auction(
                    player,
                    &inputs,
                    auction,
                    world.proposal_identity,
                    [0; 104],
                )
                .map(|_| 0)
            } else if event == crate::ai_dispatch::Event::Helper(0x468fc0) {
                let target = {
                    let players: Vec<_> = peers.iter().map(|p| p.unwrap_or(player)).collect();
                    let references: Vec<_> = players.iter().map(|p| p.reference).collect();
                    let inputs = crate::ai_takeover::World {
                        categories: world.categories,
                        buildings: world.buildings,
                        businesses: world.businesses,
                        players: &players,
                        cash_statuses: world.cash_statuses,
                        time: world
                            .strategy_time
                            .ok_or("AI takeover calendar unresolved")?,
                        image,
                    };
                    let prices = crate::business_takeover::PriceWorld {
                        categories: world.categories,
                        buildings: world.buildings,
                        businesses: world.businesses,
                        player_references: &references,
                    };
                    crate::ai_takeover::select(player, &inputs, |id| {
                        crate::business_takeover::price(
                            id,
                            &prices,
                            world
                                .ownership_tick
                                .ok_or("AI takeover ownership clock unresolved")?,
                            image,
                        )
                    })?
                };
                if let Some(business) = target {
                    let row = world
                        .businesses
                        .get(business as usize)
                        .ok_or("AI takeover selected business missing")?;
                    let index =
                        i32::from(i16::from_le_bytes(row[8..10].try_into().unwrap())) as usize;
                    let building = word(
                        world
                            .buildings
                            .get(index)
                            .ok_or("AI takeover building missing")?,
                        0,
                    );
                    callback(Call::TakeoverCommand { building }, player, profiles, world)?;
                    world.ownership.mutate(
                        player,
                        |size| allocate(world.allocation_next, size),
                        |state| {
                            state.insert(0, business as i32, |id| {
                                crate::ai_ownership::live_entry_value_from_catalog(
                                    id,
                                    world.businesses,
                                    world.buildings,
                                    world.categories,
                                    world.catalog,
                                )
                            })?;
                            state.lists[1].remove(business as i32);
                            Ok(())
                        },
                    )?;
                }
                Ok(0)
            } else if event == crate::ai_dispatch::Event::Helper(0x4687e0) {
                crate::ai_actions::expansion::clear_space(
                    player,
                    world.category_footprints,
                    &mut crate::ai_maintenance::World {
                        building_parameters: world.building_parameters,
                        image,
                        profiles,
                        categories: world.categories,
                        buildings: world.buildings,
                        businesses: world.businesses,
                        catalog: world.catalog,
                        entries: &[],
                        counts: world.category_counts,
                        identity: world.proposal_identity,
                        prior: [0; 104],
                    },
                )
                .map(|_| 0)
            } else if event == crate::ai_dispatch::Event::Helper(0x46eda0) {
                let context = crate::ai_actions::performance::World {
                    catalog: world.catalog,
                    products: world.products,
                    commodities: world.commodities,
                    categories: world.categories,
                    buildings: world.buildings,
                    businesses: world.businesses,
                    enabled: world.utility_enabled,
                    image,
                };
                // Unknown building discriminators only issue the native debug message;
                // the original then grades zero revenue/zero cost as100.
                crate::ai_actions::performance::refresh(player, world.ownership, &context, |_| {
                    Ok(())
                })
                .map(|_| 0)
            } else if event == crate::ai_dispatch::Event::Helper(0x471d00) {
                crate::ai_space::refresh_all(world.category_footprints, world.categories, image)
                    .map(|_| 0)
            } else if event == crate::ai_dispatch::Event::Helper(0x468de0) {
                business_kind(player, profiles, image, peers, world).map(u32::from)
            } else if matches!(
                event,
                crate::ai_dispatch::Event::Helper(0x467dc0 | 0x467c30)
            ) {
                maintain_businesses(
                    event == crate::ai_dispatch::Event::Helper(0x467c30),
                    player,
                    profiles,
                    image,
                    peers,
                    world,
                )
                .map(|_| 0)
            } else if event == crate::ai_dispatch::Event::ChooseBusiness {
                let mut outputs = [choices[1] as u32, choices[0] as u32];
                let result = planning(
                    player,
                    profiles,
                    image,
                    crate::ai_actions::Input {
                        execute: 1,
                        style_override: 0,
                        style: 0,
                    },
                    &mut outputs,
                    peers,
                    world,
                    planning_diagnostic,
                );
                choices[1] = outputs[0] as i32;
                choices[0] = outputs[1] as i32;
                result.map(u32::from)
            } else {
                callback(Call::Management(event), player, profiles, world).map(|reply| reply.value)
            };
            *flags = word(&player.bytes, 0x220);
            if *flags & 0x40 != 0 {
                *blocked = world.auction_blocked.ok_or("AI auction gate unresolved")?;
            }
            result
        },
    );
    crate::player_manager::put(&mut player.bytes, 0x220, flags);
    result
}

pub fn advance(
    slot: usize,
    manager: &mut Manager,
    profiles: &mut [[u8; 0x650]],
    controls: &mut ai_tick::Controls,
    image: &PeImage<'_>,
    world: &mut World<'_>,
    mut callback: impl FnMut(
        Call,
        &mut Player,
        &mut [[u8; 0x650]],
        &mut World<'_>,
    ) -> Result<ai_messages::Reply, String>,
) -> Result<(), String> {
    if slot >= manager.players.len() {
        return Err("normal AI player leaves roster".into());
    }
    let broadcast_enabled = word(&manager.bytes, 12);
    let (before, rest) = manager.players.split_at_mut(slot);
    let (player, after) = rest.split_first_mut().unwrap();
    ai_tick::advance(
        player,
        profiles,
        controls,
        image,
        |call, player, profiles, controls| match call.address {
            0x467690 => {
                crate::ai_strategy::continue_strategy(player,profiles,|call,player,profiles|{
                    match call {
                        crate::ai_strategy::ContinuationCall::Viable=>Ok(u32::from(crate::ai_guards::viable(player,image)?)),
                        crate::ai_strategy::ContinuationCall::Action(address)=>{
                            let hour=world.strategy_time.ok_or("AI continuation clock missing")?.hour;
                            let mut targets=std::mem::take(world.liquidation_targets);
                            let mut state=crate::ai_actions::continuation::World{hour,cash_statuses:world.cash_statuses,targets:&mut targets};
                            let result=crate::ai_actions::continuation::advance(address,player,profiles,image,&mut state,|call,player,profiles,state|{
                                let a=call.arguments;let mut reply=crate::ai_actions::continuation::Reply::default();
                                reply.value=match call.address {
                                    0x4d5d00=>callback(Call::Strategy(crate::ai_strategy::Call::Script{address:a[0],character:a[2]}),player,profiles,world)?.value,
                                    0x46b6c0=>{crate::ai_actions::queries::mark_category(player,a[0]);0},
                                    0x4727d0=>crate::ai_actions::queries::trouble_hours(player,state.cash_statuses,state.hour)? as u32,
                                    0x47d700|0x47d990=>crate::ai_actions::queries::limit(player,profiles,call.address==0x47d990)?,
                                    0x4698c0=>{let mut owner=a[1];let mut amount=a[2];let found=crate::ai_actions::queries::pending_offer(player,a[0],&mut owner,&mut amount)?;reply.outputs=[owner,amount];u32::from(found)},
                                    0x47aea0=>{let mut mode=a[1];let selected=neighbor_target(player,profiles,image,world,&mut mode)?;reply.outputs[0]=mode;u32::from(selected)},
                                    0x4739f0=>{let result=initial_target(player,profiles,image,world,a[1])?;reply.outputs[0]=result.output;result.value},
                                    0x477860=>u32::from(crate::ai_targets::landscape_limit(world.categories.get(a[0] as usize).ok_or("AI continuation landscape category missing")?,world.buildings)?),
                                    0x46a8d0=>{crate::ai_actions::liquidation::remove(player,a[0],state.targets)?;0},
                                    0x462f30=>crate::ai_actions::liquidation::category(a[0],world.businesses,world.buildings,world.categories)? as i32 as u32,
                                    0x478410|0x478570=>u32::from(transaction_proposal(call.address,player,profiles,[a[0],a[1]],image,world)?),
                                    0x4753f0=>if word(&player.bytes,0x13ec) as i32>0{word(&player.bytes,0x13e8)}else{0},
                                    0x4e6960=>{planning_diagnostic(crate::ai_actions::Call{address:call.address,arguments:[a[0],a[1],a[2],0,0,0,0,0,0,0],count:call.count},player,profiles)?;0},
                                    _=>callback(Call::ContinuationQuery(call),player,profiles,world)?.value,
                                };Ok(reply)
                            });
                            *world.liquidation_targets=targets;
                            result?;Ok(0)
                        },
                        crate::ai_strategy::ContinuationCall::Prepare{category,amount}=>transaction_proposal(0x4784c0,player,profiles,[category,amount],image,world).map(u32::from),
                    }
                })?;
                Ok(0)
            }
            0x47b240 => {
                let peers: Vec<_> = before.iter().map(Some).chain(std::iter::once(None)).chain(after.iter().map(Some)).collect();
                crate::ai_strategy::select(player, image, |call, player| {
                    match (call,world.strategy_time) {
                        (crate::ai_strategy::Call::Select,Some(time))=>{
                            crate::ai_selection::select(player,profiles,image,time,|call,player,profiles|{
                                let value=match call.address {
                                    0x466fa0=>{finance(player,&peers,profiles,image,world,controls.normal_tick,call.arguments[0] as u8!=0)?;0},
                                    0x476a70=>u32::from(crate::ai_strategy_queries::any_category_flag(world.categories)),
                                    0x467820=>crate::ai_scoring::choose(player,&peers,image,world.strategy_mode,|query,player|score_query(query,player,&peers,image,world))?,
                                    0x4727d0=>crate::ai_actions::queries::trouble_hours(player,world.cash_statuses,time.hour)? as u32,
                                    0x47b950=>u32::from(crate::ai_actions::queries::liquidation_needed(player,profiles,world.cash_statuses,time.hour,image)?),
                                    0x47d700=>crate::ai_actions::queries::limit(player,profiles,false)?,
                                    0x4739f0=>return initial_target(player,profiles,image,world,call.arguments[1]),
                                    0x47aea0=>{let mut mode=call.arguments[1];let selected=neighbor_target(player,profiles,image,world,&mut mode)?;return Ok(crate::ai_selection::Reply{value:u32::from(selected),output:mode});},
                                    0x47aa90=>u32::from(developed_target(player,profiles,image,&peers,world)?),
                                    0x47ab80=>{
                                        let scoring=ai_messages::offers::auction::valuation::scoring::World{image,categories:world.categories,buildings:world.buildings,businesses:world.businesses,catalog:world.catalog,
                                            railroad_income:&world.income.railroads,loan_status:world.cash_statuses,hour:time.hour,residents:world.residents,commodity_active:&world.commodity_active,before,after};
                                        let auction=world.offer_auction.map(|a|(a.category,a.active!=0));
                                        u32::from(crate::ai_acquisition::select(player,&scoring,auction,world.acquisition_checks)?)
                                    },
                                    0x46c870=>crate::ai_targets::landscape(player,|call,player|{
                                        use crate::ai_targets::{self as target,Call as T};
                                        Ok(match call {
                                            T::Active(id)=>u32::from(world.categories[id as usize][0x74]),
                                            T::Undevelopable(id)=>u32::from(target::undevelopable(image,id)?),
                                            T::LandscapeLimit(id)=>u32::from(target::landscape_limit(&world.categories[id as usize],world.buildings)?),
                                            T::OwnerCount{owner,category}=>target::owner_count(owner,&world.categories[category as usize],world.buildings,world.businesses)? as u32,
                                            T::Allowed(id)=>u32::from(target::allowed(player,id)),
                                            T::Probe{category,kind,..}=>u32::from(crate::ai_developed_targets::probe(player,category,kind,|player,outputs|planning(player,profiles,image,crate::ai_actions::Input{execute:0,style_override:0,style:0},outputs,&peers,world,planning_diagnostic))?),
                                            other=>callback(Call::Target(other),player,profiles,world)?.value,
                                        })
                                    })? as u32,
                                    0x4776c0=>{
                                        let id=call.arguments[1];
                                        // The fallback mutates the player's RNG, not the traversed
                                        // tables. Finish the read-only traversal before that callback.
                                        let mut needs_fallback=false;
                                        let value=crate::ai_targets::perimeter_preference(player,id,
                                            world.categories.get(id as usize).ok_or("AI perimeter category missing")?,world.buildings,world.businesses,
                                            |_,_|{needs_fallback=true;Ok(0)})?;
                                        if needs_fallback {
                                            crate::ai_targets::vacant_edge(player,&world.categories[id as usize]) as u32
                                        }else{value as u32}
                                    },
                                    _=>{
                                        let reply=callback(Call::Selection(call),player,profiles,world)?;
                                        return Ok(crate::ai_selection::Reply{value:reply.value,output:reply.offer});
                                    }
                                };
                                Ok(crate::ai_selection::Reply{value,output:0})
                            })?;
                            Ok(0)
                        }
                        (crate::ai_strategy::Call::DevelopmentStrategy,Some(time))=>{
                            let window=crate::ai_selection::windows(player,profiles,time.hour)?;
                            crate::ai_strategy::development_in_window(player,image,window,|call,player|{
                                callback(Call::Strategy(call),player,profiles,world).map(|r|r.value)
                            })
                        },
                        (crate::ai_strategy::Call::PrepareAuction,_)=>portfolio_value(player,profiles,image,world).map(|v|v as u32),
                        _=>callback(Call::Strategy(call), player, profiles, world).map(|r| r.value),
                    }
                })?;
                Ok(0)
            }
            0x47b3e0 => Ok(u32::from(crate::ai_strategy::initiate(
                player,
                image,
                |call, player| {
                    if call==crate::ai_strategy::Call::Finalize {liquidation_finalize(player,profiles,image,world)?;Ok(0)}
                    else if let crate::ai_strategy::Call::CategoryPrice(id)=call {Ok(ai_messages::offers::execution::category_value(world.categories.get(id as usize).ok_or("AI acquisition category missing")?,image)?.ftol_low_word() as u32)}
                    else {callback(Call::Strategy(call), player, profiles, world).map(|r| r.value)}
                },
            )?)),
            0x4779b0 => {
                offer_predecision(player,call.arguments,&mut OfferPeers{before,after},profiles,image,world,&mut callback)?;Ok(0)
            }
            0x4675f0 => Ok(u32::from(ai_messages::offers::service(player,|call,player| {
                let ai_messages::offers::Call::Execute(reference)=call else {
                    let result=callback(Call::OfferAdmission(call),player,profiles,world)?;
                    return Ok(ai_messages::offers::Reply{value:result.value,..Default::default()});
                };
                use ai_messages::offers::execution;
                // Borrow the actual retained allocation; restore the loan even
                // when a synchronous callback fails after preceding stores.
                let mut retained=world.auction_state.take();
                let result={
                let mut state=execution::World{category_owners:world.categories.iter().map(|c|c[5] as i8).collect(),buildings:world.buildings,
                    auction:retained.as_deref_mut().filter(|a|a.reference!=0).map(|a|(&mut a.record[..128]).try_into().unwrap()),colors:world.colors,event_gate:controls.event_gate};
                execution::execute(player,reference,&mut state,|call,player,state| {
                    let value=match call {
                        execution::Call::Producer{address:address @ (0x4644a0|0x4644f0|0x464530),arguments,..}=>{
                            let routing=world.message_routing.ok_or("auction action network policy unresolved")?;
                            if routing.network[0] && !routing.network[1]{return Err("auction action requires unresolved network command transport".into());}
                            let descriptor=ai_messages::offers::auction::action_descriptor(address,arguments[0],arguments[1],[0;32])?;
                            ai_messages::offers::auction::apply(ai_messages::offers::auction::Command{action:word(&descriptor,20),participant:word(&descriptor,24),amount:word(&descriptor,28)},
                                state.auction.as_deref_mut().ok_or("auction action has no retained controller")?,(before.len()+1+after.len()) as i32,true,
                                |call,_|callback(Call::AuctionAction(call),player,profiles,world).map(|_|()))?;0
                        },
                        execution::Call::Producer{address:0x48ac20,arguments,count:11}=>{
                            if world.message_routing.is_some_and(|r|r.network[0] && !r.network[1]) {
                                return Err("AI construction requires unresolved network command transport".into());
                            }
                            let references:Vec<_>=before.iter().map(|p|p.reference).chain(std::iter::once(player.reference)).chain(after.iter().map(|p|p.reference)).collect();
                            let mut context=crate::ai_actions::retained::World{building_parameters:world.building_parameters,
                                categories:world.categories,buildings:world.buildings,catalog:world.catalog,
                                products:world.products,commodities:world.commodities,residents:world.residents,
                                player_references:&references,player_count:world.active_player_count as i32,
                                category_counts:world.category_counts,proposal_identity:world.proposal_identity,
                                construction_components:world.construction_components,
                                hour:world.strategy_time.ok_or("AI construction clock unresolved")?.hour,
                                mode:world.strategy_mode as u32,allowance:world.construction_allowance,
                                affordability_override:world.affordability_override,discount:world.construction_discount,
                                proposal_prior:[0;104],
                            };
                            let descriptor=crate::ai_actions::construction::produce(player,arguments,[0;60],&mut context,image)?;
                            let command=crate::construction::Command::from_packet(&descriptor)?;
                            let routing=world.message_routing.ok_or("AI construction message routing unresolved")?;
                            let mut arena=crate::player_messages::Arena{next:*world.allocation_next};
                            let mut external=None;
                            let admitted=crate::construction::admit(&command,world.categories,
                                &mut crate::player_messages::SplitPlayers{before,current:player,after},
                                &mut arena,broadcast_enabled,routing,
                                |route,packet|{external=Some((route,*packet));Ok(())});
                            //Keep committed allocation/inbox mutations even on an error.
                            *world.allocation_next=arena.next;
                            let admitted=admitted?;
                            if let Some((route,packet))=external{
                                callback(Call::MessageRoute{route,packet},player,profiles,world)?;
                            }
                            if admitted {
                                callback(Call::ConstructionCommand{descriptor},player,profiles,world)?;
                            }
                            0
                        },
                        execution::Call::AuctionBudget=>ai_messages::offers::auction::budget::budget(player,state.auction.as_deref().ok_or("auction bid budget has no retained record")?),
                        execution::Call::Placement{category,rectangle}=>u32::from(execution::placement(
                            world.categories.get(category as usize).ok_or("AI placement category unresolved")?,rectangle)),
                        execution::Call::CategoryValue(category)=>return Ok(execution::Reply {
                            extended:execution::category_value(world.categories.get(category as usize).ok_or("AI property value category unresolved")?,image)?,value:0,
                        }),
                        execution::Call::RemoveOwnedBusiness(business)=>{
                            world.ownership.mutate(player,|_|Err("AI ownership removal attempted allocation".into()),|state|{
                                state.lists[0].remove(business as i32);Ok(())
                            })?;0
                        },
                        execution::Call::Producer { address: address @ (0x48a640 | 0x48a690), arguments, count: 3 } => {
                            let args = ai_messages::offers::producers::property(address,player.bytes[0x1bc],arguments[0],arguments[1],arguments[2],world.categories)?;
                            let routing = world.message_routing.ok_or("AI property producer message routing unresolved")?;
                            let mut arena = crate::player_messages::Arena { next: *world.allocation_next };
                            // External transport callbacks cannot reborrow the manager or
                            // current player while the split is live. Capture the route,
                            // then invoke it before the producer returns and mutates its offer.
                            let mut external = None;
                            let result = crate::player_messages::produce_in(args,broadcast_enabled,
                                &mut crate::player_messages::SplitPlayers { before, current: player, after },
                                &mut arena,routing,|route,packet| { external=Some((route,*packet)); Ok(()) });
                            *world.allocation_next=arena.next;
                            result?;
                            if let Some((route,mut packet))=external {
                                if route==crate::player_messages::Route::Broadcast && matches!(word(&packet,4),0x22|0x23) {
                                    // Explicit zero prior policy for the original uninitialized
                                    // broadcast command stack. Meaningful fields are native stores.
                                    ai_messages::offers::auction::broadcast::dispatch(&mut packet,broadcast_enabled as i32,[0;52],|call,packet|{
                                        match call {
                                            ai_messages::offers::auction::broadcast::Call::Queue{descriptor,..}=>{
                                                callback(Call::AuctionCommand{descriptor,additional_delay:0},player,profiles,world)?;
                                            },
                                            ai_messages::offers::auction::broadcast::Call::Reply=>{
                                                let mut arena=crate::player_messages::Arena{next:*world.allocation_next};let mut external=None;
                                                let result=crate::player_messages::reply_in(packet,0x24,[0x5f5e0ff;4],
                                                    &mut crate::player_messages::SplitPlayers{before,current:player,after},&mut arena,routing,
                                                    |route,packet|{external=Some((route,*packet));Ok(())});
                                                *world.allocation_next=arena.next;result?;
                                                if let Some((route,packet))=external{callback(Call::MessageRoute{route,packet},player,profiles,world)?;}
                                            }
                                        }Ok(())
                                    })?;
                                }else{callback(Call::MessageRoute { route,packet },player,profiles,world)?;}
                            }
                            0
                        },
                        other=>callback(Call::OfferExecution(other),player,profiles,world)?.value,
                    };Ok(execution::Reply{value,..Default::default()})
                })
                };
                world.auction_state=retained;
                if let Some(a)=world.auction_state.as_deref().filter(|a|a.reference!=0){
                    world.auction=Some(ai_messages::Auction{participant:word(&a.record,0x54),winner:word(&a.record,0x60) as i32,price:word(&a.record,0x6c) as i32});
                    world.offer_auction=Some(ai_messages::offers::Auction{category:word(&a.record,4),active:a.record[0x7d]});
                    world.auction_blocked=Some(a.record[0x7d]!=0);
                }
                result?;
                Ok(Default::default())
            })?)),
            0x47b950 => Ok(u32::from(crate::ai_actions::queries::liquidation_needed(player,profiles,world.cash_statuses,world.strategy_time.ok_or("AI liquidation clock unresolved")?.hour,image)?)),
            0x47b8f0 => Ok(u32::from(crate::ai_guards::viable(player, image)?)),
            0x47b870 => {
                if world.auction.is_some() && world.auction_guard.is_none() {
                    return Err("AI auction progress context unresolved".into());
                }
                Ok(u32::from(crate::ai_guards::auction(
                    player,
                    profiles,
                    world.auction_guard,
                )?))
            }
            0x466870 => {
                player.reset_ai(image, world.global_reset, &mut |reference| {
                    world
                        .ownership
                        .lists
                        .remove(&reference)
                        .ok_or("AI reset ownership allocation unresolved")?;
                    Ok(())
                })?;
                Ok(0)
            }
            0x468670 => {
                dispatch(player, profiles, image, before, after, world, &mut callback)?;
                Ok(0)
            }
            0x467370 | 0x467440 => {
                let secondary = call.address == 0x467370;
                crate::ai_events::consume_player(
                    player,
                    profiles,
                    controls.day_tick as i32,
                    secondary,
                    &world.categories.map(|c|c[0x6c]),
                    |code, player, profiles| {
                        callback(
                            Call::TimetableDiagnostic { secondary, code },
                            player,
                            profiles,
                            world,
                        )
                        .map(|_| ())
                    },
                )?;
                Ok(0)
            }
            0x466fa0 => {
                let peers: Vec<_> = before.iter().map(Some)
                    .chain(std::iter::once(None)).chain(after.iter().map(Some)).collect();
                finance(player,&peers,profiles,image,world,controls.normal_tick,call.arguments[0] as u8!=0)?;
                Ok(0)
            }
            0x47d6a0 => Ok(if word(&player.bytes, 0x150) < 10 {
                controls.normal_tick.wrapping_add(125)
            } else {
                0
            }),
            0x469210 => {
                let mut context = ai_messages::Context {
                    image,
                    profiles,
                    buildings: world.buildings,
                    auction: world.auction,
                };
                ai_messages::consume(player, &mut context, |call, player, context| {
                    use ai_messages::offers::auction::inbox;
                    let value=match call.address {
                        0x46ae20=>{
                            world.ownership.mutate(player,|_|Err("AI inbox ownership removal attempted allocation".into()),|state|{
                                state.lists.get_mut(call.arguments[1] as usize).ok_or("AI inbox ownership list mode invalid")?.remove(call.arguments[0] as i32);Ok(())
                            })?;0
                        },
                        0x4779b0=>{offer_predecision(player,[call.arguments[0],call.arguments[1]],&mut OfferPeers{before,after},context.profiles,image,world,&mut callback)?;0},
                        0x476920=>u32::from(crate::ai_strategy_queries::utility_or_transport(image,call.arguments[0])?),
                        0x478e00=>inbox::ordinary(player,call.arguments[0])?,
                        0x478950=>{inbox::cleanup(player)?;0},
                        0x46aa40=>{
                            world.ownership.mutate(player,|size|allocate(world.allocation_next,size),|state|{
                                state.insert(call.arguments[1] as usize,call.arguments[0] as i32,|id|crate::ai_ownership::live_entry_value_from_catalog(id,world.businesses,world.buildings,world.categories,world.catalog))
                            })?;0
                        },
                        0x472620=>{crate::ai_business::remove_category_count(world.category_counts,call.arguments[0],call.arguments[1])?;0},
                        0x47d730=>inbox::history_age(player,context.profiles)?,
                        0x445cc0=>{
                            let offset=call.object.checked_sub(0x617628).ok_or("auction wait category precedes pool")?;
                            if offset%380!=0{return Err("auction wait category is unaligned".into());}
                            let category=world.categories.get((offset/380) as usize).ok_or("auction wait category leaves pool")?;
                            let routing=world.message_routing.ok_or("auction wait command routing unresolved")?;
                            if routing.network[0]&&!routing.network[1]{return Err("auction wait command requires network transport".into());}
                            let mut descriptor=[0;52];descriptor[..24].copy_from_slice(&inbox::wait_descriptor(category,[0;24]));
                            callback(Call::AuctionCommand{descriptor,additional_delay:0},player,context.profiles,world)?;0
                        },
                        0x4789b0=>{
                            let packet=ai_messages::offers::packet(player,call.arguments[1])?;
                            let offer=inbox::winning(player,&packet)?;
                            return Ok(ai_messages::Reply{value:u32::from(offer.is_some()),offer:offer.unwrap_or(0)});
                        },
                        0x4699c0=>{
                            let packet=ai_messages::offers::packet(player,call.arguments[1])?;
                            let auction=world.auction_state.as_deref().filter(|a|a.reference!=0).ok_or("auction inbox controller unresolved")?;
                            let loan=*world.cash_statuses.get(player.bytes[0x1bc] as usize).ok_or("auction inbox loan slot unresolved")?!=0;
                            let mut state=inbox::World{auction:auction.record[..128].try_into().unwrap(),categories:world.categories,loan,image,identity:world.proposal_identity};
                            // Explicit host policy for bytes the native local constructor leaves untouched.
                            inbox::activate(player,call.arguments[0],&packet,[0;104],&mut state)?;0
                        },
                        _=>return callback(Call::Message(call), player, context.profiles, world),
                    };
                    Ok(ai_messages::Reply{value,offer:0})
                })?;
                Ok(0)
            }
            _ => callback(Call::Decision(call), player, profiles, world).map(|reply| reply.value),
        },
    ).map_err(|error|format!("normal AI slot{slot}: {error}"))
}

#[cfg(test)]
mod income_tests;
