//! Live local construction/business commands over the retained city.
pub use crate::business_open::Command as OpenBusiness;
use crate::{
    business_binding, business_pool, construction, economy,
    level_startup::{City, Roster},
    pe::PeImage,
};
#[derive(Clone)]
pub enum Command {
    /// Original property/auction command16/19/1d/1e/1f/20 descriptor.
    Auction {
        descriptor: [u8; 52],
    },
    CategoryReady {
        category: u32,
    },
    PropertyTransfer {
        category: i16,
        owner: u8,
        amount: i32,
    },
    EventCard(crate::event_cards::lifecycle::Command),
    Eliminate {
        slot: usize,
    },
    OutcomeNotice {
        player: i32,
        reason: u32,
    },
    Build(construction::Command),
    Demolish(crate::demolition::Command),
    Open(OpenBusiness),
    SellBusiness(crate::business_sale::Command),
    Takeover(crate::business_takeover::Command),
    Convert(crate::business_conversion::Command),
    Terms(economy::PendingBusinessTerms),
}
pub type Queue = crate::local_commands::Queue<Command>;

/// Snapshot original445720's live quote into a command. Placement/admission
/// policy belongs to the submitting UI/AI; the original consumer does not
/// rerun that policy after the25-tick delay.
pub fn quote(
    command: &mut construction::Command,
    city: &mut City,
    roster: &Roster,
    image: &PeImage<'_>,
    hour: u32,
) -> Result<i32, String> {
    let category = city
        .categories
        .get(command.category as usize)
        .ok_or("quote category leaves city")?;
    let owner = roster
        .manager
        .players
        .get((i32::from(command.owner) - 1) as usize)
        .ok_or("quote owner leaves roster")?
        .reference;
    let mut utility_owners = [0; 4];
    for (result, index) in utility_owners.iter_mut().zip([2, 35, 39, 42]) {
        let id = city.categories[index][5];
        if id != 0 {
            *result = roster
                .manager
                .players
                .get(usize::from(id) - 1)
                .ok_or("quote utility owner unresolved")?
                .reference;
        }
    }
    let info = city
        .catalog
        .info
        .row(command.business as usize)
        .ok_or("quote business type leaves catalog")?;
    let mut products = Vec::new();
    if hour >= 6 && command.kind == 0 {
        let entries = city.catalog.products[command.business as usize]
            .get(..usize::from(info[0x4c]))
            .ok_or("quote stock catalog exceeds six")?;
        for entry in entries {
            let row = city
                .commodities
                .rows
                .get(entry[0] as usize)
                .ok_or("quote commodity outside catalog")?;
            products.push(crate::building_binding::StockProduct {
                value: f32::from_le_bytes(row[0x14..0x18].try_into().unwrap()),
                fraction: f32::from_bits(entry[1]),
            });
        }
    }
    let [x0, y0, x1, y1] = command.rectangle.map(i32::from);
    let cost = quote_components(
        command,
        &crate::building_binding::ConstructionInputs {
            precision: crate::x87::Precision::Pc64,
            volume: (x1 - x0 + 1)
                .wrapping_mul(y1 - y0 + 1)
                .wrapping_mul(i32::from(command.height)),
            kind: command.kind,
            quality: command.quality,
            category_value: f32::from_le_bytes(category[0x84..0x88].try_into().unwrap()),
            hour: hour as i32,
            discount: image.f32_at(0x53be78)?,
            owner,
            utility_owners,
            business: info,
            products: &products,
        },
        &city.building_parameters,
        image,
    )?;
    city.construction_components = cost.components;
    Ok(cost.total)
}

/// Shared command producer boundary: snapshot all four original quote globals
/// before enqueueing. In-play producers call this with PC64 inputs.
pub fn quote_components(
    command: &mut construction::Command,
    input: &crate::building_binding::ConstructionInputs<'_>,
    parameters: &crate::building_parameters::BuildingKinds,
    image: &PeImage<'_>,
) -> Result<crate::building_binding::ConstructionCost, String> {
    let cost =
        crate::building_binding::construction_cost_with_parameters(input, parameters, image)?;
    command.components = cost.components;
    Ok(cost)
}
pub struct Context<'a, 'image> {
    pub image: &'a PeImage<'image>,
    pub clock: &'a crate::clock::Clock,
    pub accounting_enabled: bool,
    /// Original801dec: demand cache initialized by4717c0. This is separate
    /// from the host preference to display interface notifications.
    pub notifications: bool,
    /// Original optional86d338 object+85c tutorial-event gate.
    pub tutorial_state: u8,
    pub network_enabled: bool,
    pub network_override: bool,
    pub utility_factor: f32,
}
pub struct CommandContext<'a, 'image> {
    pub image: &'a PeImage<'image>,
    pub clock: &'a mut crate::clock::Clock,
    pub accounting_enabled: bool,
    /// Original801dec: demand cache initialized by4717c0. This is separate
    /// from the host preference to display interface notifications.
    pub notifications: bool,
    /// Original optional86d338 object+85c tutorial-event gate.
    pub tutorial_state: u8,
    pub network_enabled: bool,
    pub network_override: bool,
    pub utility_factor: f32,
}
impl CommandContext<'_, '_> {
    /// Reborrow the actual clock for one native consumer; never snapshot its words.
    pub fn read(&self) -> Context<'_, '_> {
        Context {
            image: self.image,
            clock: &*self.clock,
            accounting_enabled: self.accounting_enabled,
            notifications: self.notifications,
            tutorial_state: self.tutorial_state,
            network_enabled: self.network_enabled,
            network_override: self.network_override,
            utility_factor: self.utility_factor,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    AccountPosted(economy::AccountPosting),
    PropertyOffer(crate::property_offer::Call),
    LeaseNotification(crate::category::Call),
    /// Original465490 visual/interface prefix; return4b5370 prior interface mode.
    /// Return the resolved prior interface mode. Original mode4 first closes
    /// the modal through43c690 and restores its retained86da44 mode.
    AuctionScenePrepare,
    AuctionActivation(crate::ai_messages::offers::auction::activation::Call),
    AuctionAction(crate::ai_messages::offers::auction::Call),
    /// Native ShowPanel565ba8 carries its synchronous current-player cash.
    AuctionTimeShow {
        cash: f32,
    },
    AuctionClosure(crate::ai_messages::offers::auction::closure::Call),
    AuctionNetworkBarrier,

    AuctionAdmission(crate::ai_messages::offers::auction::admission::Call),
    AuctionQueue {
        descriptor: [u8; 52],
        additional_delay: u32,
    },
    CategoryReadyAt {
        category: u32,
        normal_tick: u32,
    },
    PropertyOwnership(crate::category_ownership::Seam),
    PropertyTransferNotice {
        category: i16,
        owner: u8,
        amount: i32,
    },
    ConstructionNotice {
        building: u32,
        owner: u32,
        structure_kind: u32,
        business: u32,
        business_kind: u32,
        category: u32,
    },
    /// Original4504e0 type3 notification after the demolition queue producer.
    DemolitionNotice {
        owner: i8,
        building: u16,
        structure_kind: u32,
        category: i8,
    },
    PropertyView(i16),
    EventCardDiagnostic {
        kind: u32,
        used: bool,
    },
    EventCardNotice(crate::event_cards::effects::Notice),
    EventCardTargeted(crate::event_cards::targeted::Call),
    Elimination(crate::player_elimination::Call),
    OutcomeNotice {
        player: i32,
        reason: u32,
    },
    Outcome(crate::outcome_notice::Call),
    PlayerMessageRoute {
        route: crate::player_messages::Route,
        packet: crate::player_messages::Packet,
    },
    Construction(construction::Event),
    Business(business_binding::Call),
    RefreshInterface,
    OpenCall(crate::business_open::Call),
    BusinessSale(crate::business_sale::Call),
    BusinessTakeover(crate::business_takeover::Call),
    Demolition(crate::demolition::Call),
    Detach(crate::business_detach::Call),
}

pub fn auction_action_event(call: crate::ai_messages::offers::auction::Call, cash: f32) -> Event {
    if call == crate::ai_messages::offers::auction::Call::ShowPanel(0x565ba8) {
        Event::AuctionTimeShow { cash }
    } else {
        Event::AuctionAction(call)
    }
}

/// Execute the admitted card command against live accounts and targeted pools.
/// Callbacks observe completed mutations in original order, including postings
/// before ownership transfer. The caller owns message delivery and notice UI.
pub fn event_card(
    command: &crate::event_cards::lifecycle::Command,
    deck: &mut crate::event_cards::Deck,
    world: &mut crate::event_cards::effects::World<'_>,
    context: &mut crate::event_cards::targeted::Context<'_, '_>,
    utilities: &mut economy::Utilities,
    mut event: impl FnMut(Event, &mut crate::event_cards::effects::World<'_>) -> Result<(), String>,
) -> Result<crate::event_cards::effects::Notice, String> {
    use crate::event_cards::{effects, targeted};
    effects::apply(
        command,
        deck,
        world,
        context.image,
        |call, world, notice| match call {
            effects::Call::AccountPosted(posting) => {
                event(Event::AccountPosted(posting), world)?;
                Ok(true)
            }
            effects::Call::Utility {
                kind,
                amount,
                player,
            } => effects::utility_rebate(
                kind,
                amount,
                player,
                world,
                utilities,
                context.catalog,
                notice,
                |call, world, _| {
                    let effects::Call::AccountPosted(posting) = call else {
                        return Err("utility rebate emitted a non-posting effect".into());
                    };
                    event(Event::AccountPosted(posting), world)?;
                    Ok(true)
                },
            ),
            effects::Call::Target {
                kind,
                target,
                player,
            } => targeted::apply(
                kind,
                target,
                player,
                world,
                context,
                notice,
                |call, world| {
                    let event_call = match call {
                        targeted::Call::AccountPosted(posting) => Event::AccountPosted(posting),
                        call => Event::EventCardTargeted(call),
                    };
                    event(event_call, world)
                },
            ),
        },
    )
}

/// Apply a locally admitted descriptor to the same city used by shopping,
/// housing and reports. Derived topology is rebuilt after structural changes.
pub struct Runtime<'a> {
    pub event_cards: &'a mut crate::event_cards::Deck,
    pub message_context: &'a mut crate::player_messages::Context,
    pub city: &'a mut City,
    pub roster: &'a mut Roster,
    pub population: &'a mut crate::level_runtime::Population,
    pub utilities: &'a mut economy::Utilities,
    pub board: &'a crate::board::Board,
}
/// Original41d796 calls48dc40 synchronously, before announcement visibility,
/// countdown and category-lock stores. The retained script owner stays in host.
pub trait TutorialAnnouncement {
    fn control(
        &mut self,
        control: u32,
        _state: &mut crate::level_script::State<'_>,
        _geometry: &mut dyn crate::building_pool::GeometryBinding,
        _utilities: &mut economy::Utilities,
    ) -> Result<(), String> {
        Err(format!(
            "native tutorial callback {control} has no script owner"
        ))
    }
    fn start(
        &mut self,
        state: &mut crate::level_script::State<'_>,
        presentation: &crate::ai_messages::offers::auction::admission::Presentation,
        geometry: &mut dyn crate::building_pool::GeometryBinding,
        utilities: &mut economy::Utilities,
    ) -> Result<(), String>;
}
impl Runtime<'_> {
    fn current_player_cash(&self) -> Result<f32, String> {
        let slot = self
            .roster
            .manager
            .players
            .iter()
            .position(|player| player.reference == self.roster.status.current_player)
            .ok_or("auction Time current player missing")?;
        self.city
            .ledgers
            .players
            .get(slot)
            .map(|player| player.cash)
            .ok_or_else(|| "auction Time current account missing".into())
    }
    /// Start/action/finish consumers need mutable mode822d30. The session uses
    /// this entrypoint before constructing the read-only normal-command context.
    pub fn auction_phase(
        &mut self,
        d: &[u8; 52],
        clock: &mut crate::clock::Clock,
        network: [bool; 2],
        event: impl FnMut(Event, &crate::clock::Clock) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.auction_phase_with_tutorial(d, clock, network, None, event)
    }
    pub fn auction_phase_with_tutorial(
        &mut self,
        d: &[u8; 52],
        clock: &mut crate::clock::Clock,
        network: [bool; 2],
        mut tutorial: Option<(
            u8,
            &mut dyn crate::building_pool::GeometryBinding,
            &mut dyn TutorialAnnouncement,
        )>,
        mut event: impl FnMut(Event, &crate::clock::Clock) -> Result<u32, String>,
    ) -> Result<(), String> {
        use crate::{
            ai_messages::offers::auction,
            player_manager::{put, word},
        };
        match word(d, 4) {
            0x1e => {
                if self.city.auction.controller.record[0x7d] == 0 {
                    return Ok(());
                }
                self.city.auction.presentation.visible = false;
                event(
                    Event::AuctionClosure(auction::closure::Call::CloseAnnouncement),
                    clock,
                )?;
                clock.timing[5] = 2;
                let previous = event(Event::AuctionScenePrepare, clock)?;
                put(&mut self.city.auction.controller.record, 0x150, previous);
                let cash = self.current_player_cash()?;
                auction::activation::activate(
                    &mut self.city.auction.controller,
                    &mut self.city.auction.presentation,
                    &mut self.roster.manager.players,
                    &self.city.categories,
                    network[0],
                    |call, _, _| {
                        if let auction::activation::Call::Action(action) = call {
                            event(auction_action_event(action, cash), clock)
                        } else {
                            event(Event::AuctionActivation(call), clock)
                        }
                    },
                )?;
                if network[0] {
                    event(Event::AuctionNetworkBarrier, clock)?;
                }
                Ok(())
            }
            0x1f => {
                let cash = self.current_player_cash()?;
                let count = self.roster.manager.players.len() as i32;
                let players: Vec<_> = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| {
                        Some(business_binding::Player {
                            identity: p.reference,
                            kind: word(&p.bytes, 0x154),
                        })
                    })
                    .collect();
                let City {
                    auction,
                    categories,
                    ai_demand,
                    objective,
                    settings,
                    configuration,
                    welfare,
                    pause,
                    buildings,
                    ledgers,
                    definitions,
                    commodities,
                    catalog,
                    creation,
                    available,
                    availability_cursor,
                    player_money,
                    construction_components,
                    ..
                } = &mut *self.city;
                auction::apply(
                    auction::Command {
                        action: word(d, 20),
                        participant: word(d, 24),
                        amount: word(d, 28),
                    },
                    (&mut auction.controller.record[..128]).try_into().unwrap(),
                    count,
                    true,
                    |call, _| {
                        if let auction::Call::Tutorial(control) = call
                            && let Some((1, geometry, callback)) = tutorial.as_mut()
                        {
                            callback.control(
                                control,
                                &mut crate::level_script::State {
                                    clock: &mut *clock,
                                    ai_demand,
                                    objective,
                                    settings,
                                    configuration,
                                    welfare,
                                    pause,
                                    buildings,
                                    businesses: &mut ledgers.businesses,
                                    categories,
                                    definitions,
                                    commodities,
                                    catalog,
                                    residents: &mut ledgers.residents,
                                    creation,
                                    available,
                                    availability_cursor,
                                    players: &players,
                                    player_accounts: &mut ledgers.players,
                                    player_money,
                                    construction_components,
                                },
                                *geometry,
                                self.utilities,
                            )?;
                            return Ok(());
                        }
                        event(auction_action_event(call, cash), clock).map(|_| ())
                    },
                )
            }
            0x20 => {
                let players: Vec<_> = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| {
                        Some(business_binding::Player {
                            identity: p.reference,
                            kind: word(&p.bytes, 0x154),
                        })
                    })
                    .collect();
                let mut kinds = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| word(&p.bytes, 0x154))
                    .collect();
                let City {
                    auction,
                    categories,
                    ai_demand,
                    objective,
                    settings,
                    configuration,
                    welfare,
                    pause,
                    buildings,
                    ledgers,
                    definitions,
                    commodities,
                    catalog,
                    creation,
                    available,
                    availability_cursor,
                    player_money,
                    construction_components,
                    ..
                } = &mut *self.city;
                let mut world = auction::closure::World {
                    controller: &mut auction.controller,
                    categories,
                    presentation: &mut auction.presentation,
                    player_kinds: &mut kinds,
                    network,
                    command_prior: [0; 28],
                    row_text: &mut auction.participant_ui,
                    bid_text: &mut auction.bid_ui,
                };
                auction::closure::finish(&mut world, |call, world| match call {
                    auction::closure::Call::Tutorial(control)
                        if tutorial.as_ref().is_some_and(|(state, _, _)| *state == 1) =>
                    {
                        let (_, geometry, callback) = tutorial.as_mut().unwrap();
                        callback.control(
                            control,
                            &mut crate::level_script::State {
                                clock: &mut *clock,
                                ai_demand,
                                objective,
                                settings,
                                configuration,
                                welfare,
                                pause,
                                buildings,
                                businesses: &mut ledgers.businesses,
                                categories: (&mut *world.categories)
                                    .try_into()
                                    .map_err(|_| "tutorial auction closure category pool size")?,
                                definitions,
                                commodities,
                                catalog,
                                residents: &mut ledgers.residents,
                                creation,
                                available,
                                availability_cursor,
                                players: &players,
                                player_accounts: &mut ledgers.players,
                                player_money,
                                construction_components,
                            },
                            *geometry,
                            self.utilities,
                        )
                    }
                    auction::closure::Call::Message(args) => {
                        player_message(args, &mut self.roster.manager, self.message_context, |e| {
                            event(e, clock).map(|_| ())
                        })
                    }
                    auction::closure::Call::QueueTransfer(d) => {
                        let mut descriptor = [0; 52];
                        descriptor[..28].copy_from_slice(&d);
                        event(
                            Event::AuctionQueue {
                                descriptor,
                                additional_delay: 0,
                            },
                            clock,
                        )
                        .map(|_| ())
                    }
                    other => {
                        if other == auction::closure::Call::RestorePresentation {
                            clock.timing[5] = 1;
                        }
                        if other == auction::closure::Call::CloseAnnouncement {
                            world.presentation.visible = false;
                        }
                        event(Event::AuctionClosure(other), clock).map(|_| ())
                    }
                })
            }
            kind => Err(format!("command{kind:x} is not an auction phase")),
        }
    }
    pub fn advance_auction(
        &mut self,
        clock: &mut crate::clock::Clock,
        network: [bool; 2],
        event: impl FnMut(Event, &crate::clock::Clock) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.advance_auction_with_tutorial(clock, network, None, event)
    }
    pub fn advance_auction_with_tutorial(
        &mut self,
        clock: &mut crate::clock::Clock,
        network: [bool; 2],
        mut tutorial: Option<(
            u8,
            &mut dyn crate::building_pool::GeometryBinding,
            &mut dyn TutorialAnnouncement,
        )>,
        mut event: impl FnMut(Event, &crate::clock::Clock) -> Result<u32, String>,
    ) -> Result<(), String> {
        use crate::{ai_messages::offers::auction, player_manager::word};
        if self.city.auction.controller.reference == 0 {
            return Err("auction scheduler has no retained controller".into());
        }
        let cash = self.current_player_cash()?;
        let active: Vec<u8> = self
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.bytes[0x10])
            .collect();
        let players: Vec<_> = self
            .roster
            .manager
            .players
            .iter()
            .map(|p| {
                Some(business_binding::Player {
                    identity: p.reference,
                    kind: word(&p.bytes, 0x154),
                })
            })
            .collect();
        let City {
            auction: state,
            categories,
            ai_demand,
            objective,
            settings,
            configuration,
            welfare,
            pause,
            buildings,
            ledgers,
            definitions,
            commodities,
            catalog,
            creation,
            available,
            availability_cursor,
            player_money,
            construction_components,
            ..
        } = &mut *self.city;
        let mut world = auction::progress::World {
            record: (&mut state.controller.record[..128]).try_into().unwrap(),
            selected: &mut state.presentation.selected,
            active: &active,
            network,
            command_prior: [[0; 32]; 2],
            opening: &mut state.opening,
        };
        auction::progress::update(&mut world, |call, world| match call {
            auction::progress::Call::Message(args) => {
                player_message(args, &mut self.roster.manager, self.message_context, |e| {
                    event(e, clock).map(|_| ())
                })
            }
            auction::progress::Call::Queue {
                descriptor: d,
                size,
            } => {
                if !network[0] && word(&d, 4) == 0x1f && d[16] == 0 && word(&d, 12) == 0 {
                    auction::apply(
                        auction::Command {
                            action: word(&d, 20),
                            participant: word(&d, 24),
                            amount: word(&d, 28),
                        },
                        world.record,
                        active.len() as i32,
                        true,
                        |call, _| {
                            if let auction::Call::Tutorial(control) = call
                                && let Some((1, geometry, callback)) = tutorial.as_mut()
                            {
                                return callback.control(
                                    control,
                                    &mut crate::level_script::State {
                                        clock: &mut *clock,
                                        ai_demand,
                                        objective,
                                        settings,
                                        configuration,
                                        welfare,
                                        pause,
                                        buildings,
                                        businesses: &mut ledgers.businesses,
                                        categories,
                                        definitions,
                                        commodities,
                                        catalog,
                                        residents: &mut ledgers.residents,
                                        creation,
                                        available,
                                        availability_cursor,
                                        players: &players,
                                        player_accounts: &mut ledgers.players,
                                        player_money,
                                        construction_components,
                                    },
                                    *geometry,
                                    self.utilities,
                                );
                            }
                            event(auction_action_event(call, cash), clock).map(|_| ())
                        },
                    )
                } else {
                    let mut descriptor = [0; 52];
                    descriptor[..size].copy_from_slice(&d[..size]);
                    event(
                        Event::AuctionQueue {
                            descriptor,
                            additional_delay: 0,
                        },
                        clock,
                    )
                    .map(|_| ())
                }
            }
        })?;
        let _ = clock;
        Ok(())
    }
    fn apply_auction(
        &mut self,
        d: &[u8; 52],
        context: &mut CommandContext<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        mut announcement: Option<&mut dyn TutorialAnnouncement>,
        event: &mut impl FnMut(Event, &crate::clock::Clock) -> Result<(), String>,
    ) -> Result<(), String> {
        use crate::{ai_messages::offers::auction::admission, player_manager::word};
        match word(d, 4) {
            0x27..=0x2a => {
                // Auction notifications are host-produced, addressed inbox
                // messages. Only the destination's local human consumes them.
                let slot = usize::from(d[0x15]);
                let player = self
                    .roster
                    .manager
                    .players
                    .get_mut(slot)
                    .ok_or("auction notification recipient leaves roster")?;
                if player.reference == self.roster.status.current_player {
                    let packet = d[..48]
                        .try_into()
                        .map_err(|_| "auction notification extent")?;
                    crate::player_messages::enqueue(player, packet, || {
                        self.message_context.arena.allocate()
                    })?;
                }
                Ok(())
            }
            0x19 => {
                let category = word(d, 20);
                let row = self
                    .city
                    .categories
                    .get_mut(category as usize)
                    .ok_or("category wait command leaves pool")?;
                let normal_tick =
                    crate::category_ownership::begin_wait(row, context.clock.timing[1]);
                event(
                    Event::CategoryReadyAt {
                        category,
                        normal_tick,
                    },
                    &*context.clock,
                )
            }
            0x16 => {
                let result = property_transfer(
                    i16::from_le_bytes(d[24..26].try_into().unwrap()),
                    d[26],
                    word(d, 20) as i32,
                    self.city,
                    self.roster,
                    self.message_context,
                    &context.read(),
                    |call| event(call, &*context.clock),
                );
                self.utilities.owners = [2, 35, 39, 42].map(|i| self.city.categories[i][5]);
                result
            }
            0x1d => {
                let active: Vec<u8> = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| p.bytes[0x10])
                    .collect();
                let current = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .find(|p| p.reference == self.roster.status.current_player)
                    .ok_or("auction current player unresolved")?
                    .bytes[0x1bc];
                let players: Vec<_> = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| {
                        Some(business_binding::Player {
                            identity: p.reference,
                            kind: word(&p.bytes, 0x154),
                        })
                    })
                    .collect();
                // Admission owns only auction/category storage. Borrow all remaining
                // authoritative script records alongside it; no shadow/taken pools.
                let City {
                    auction,
                    categories,
                    ai_demand,
                    objective,
                    settings,
                    configuration,
                    welfare,
                    pause,
                    buildings,
                    ledgers,
                    definitions,
                    commodities,
                    catalog,
                    creation,
                    available,
                    availability_cursor,
                    player_money,
                    construction_components,
                    ..
                } = &mut *self.city;
                let mut world = admission::World {
                    controller: &mut auction.controller,
                    categories,
                    active: &active,
                    presentation: &mut auction.presentation,
                    current_slot: current,
                    tutorial: context.tutorial_state != 0,
                    network: [context.network_enabled, context.network_override],
                    command_prior: [0; 20],
                };
                let admitted = admission::admit(
                    word(d, 20),
                    word(d, 24),
                    word(d, 28),
                    &mut world,
                    |call, world| {
                        if let admission::Call::QueueStart {
                            additional_delay,
                            descriptor,
                        } = call
                        {
                            let mut bytes = [0; 52];
                            bytes[..20].copy_from_slice(&descriptor);
                            event(
                                Event::AuctionQueue {
                                    descriptor: bytes,
                                    additional_delay,
                                },
                                &*context.clock,
                            )
                        } else {
                            if call == admission::Call::ShowAnnouncement {
                                if context.tutorial_state == 1
                                    && let Some(callback) = announcement.as_deref_mut()
                                {
                                    callback.start(
                                        &mut crate::level_script::State {
                                            clock: &mut *context.clock,
                                            ai_demand,
                                            objective,
                                            settings,
                                            configuration,
                                            welfare,
                                            pause,
                                            buildings,
                                            businesses: &mut ledgers.businesses,
                                            categories: (&mut *world.categories)
                                                .try_into()
                                                .map_err(
                                                    |_| "tutorial announcement category pool size",
                                                )?,
                                            definitions,
                                            commodities,
                                            catalog,
                                            residents: &mut ledgers.residents,
                                            creation,
                                            available,
                                            availability_cursor,
                                            players: &players,
                                            player_accounts: &mut ledgers.players,
                                            player_money,
                                            construction_components,
                                        },
                                        world.presentation,
                                        geometry,
                                        self.utilities,
                                    )?;
                                }
                                world.presentation.visible = true;
                            }
                            event(Event::AuctionAdmission(call), &*context.clock)
                        }
                    },
                )?;
                if !admitted && (!context.network_enabled || context.network_override) {
                    self.message_context.produce(
                        admission::busy_reply(d),
                        &mut self.roster.manager,
                        |route, packet| {
                            if let crate::player_messages::Route::PropertyRefresh { category } =
                                route
                            {
                                crate::category_ownership::property_view(
                                    self.city
                                        .categories
                                        .get_mut(category as usize)
                                        .ok_or("busy auction reply category leaves pool")?,
                                    0,
                                    1,
                                );
                            }
                            event(
                                Event::PlayerMessageRoute {
                                    route,
                                    packet: *packet,
                                },
                                &*context.clock,
                            )
                        },
                    )?;
                }
                Ok(())
            }
            kind => Err(format!(
                "auction command{kind:x} requires retained scene consumer"
            )),
        }
    }
    pub fn apply(
        &mut self,
        command: &Command,
        context: &mut CommandContext<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        mut event: impl FnMut(Event) -> Result<(), String>,
    ) -> Result<(), String> {
        self.apply_with_announcement(command, context, geometry, None, |call, _clock| event(call))
    }
    pub fn apply_with_announcement(
        &mut self,
        command: &Command,
        context: &mut CommandContext<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        announcement: Option<&mut dyn TutorialAnnouncement>,
        mut event: impl FnMut(Event, &crate::clock::Clock) -> Result<(), String>,
    ) -> Result<(), String> {
        let result = match command {
            Command::CategoryReady { category } => {
                crate::category_ownership::end_wait(
                    self.city
                        .categories
                        .get_mut(*category as usize)
                        .ok_or("scheduled category leaves pool")?,
                );
                Ok(())
            }
            Command::Auction { descriptor } => {
                self.apply_auction(descriptor, context, geometry, announcement, &mut event)
            }
            Command::PropertyTransfer {
                category,
                owner,
                amount,
            } => {
                let result = property_transfer(
                    *category,
                    *owner,
                    *amount,
                    self.city,
                    self.roster,
                    self.message_context,
                    &context.read(),
                    |call| event(call, &*context.clock),
                );
                self.utilities.owners = [2, 35, 39, 42].map(|i| self.city.categories[i][5]);
                result
            }
            Command::EventCard(command) => {
                let active: Vec<_> = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| p.bytes[0x10])
                    .collect();
                let current = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .position(|p| p.reference == self.roster.status.current_player)
                    .ok_or("event current player outside roster")?;
                let notice = event_card(
                    command,
                    self.event_cards,
                    &mut crate::event_cards::effects::World {
                        accounts: &mut self.city.ledgers.players,
                        active: &active,
                        businesses: &mut self.city.ledgers.businesses,
                        buildings: &mut self.city.buildings,
                        categories: &self.city.categories,
                        counts: &mut self.population.business_counts,
                        current,
                    },
                    &mut crate::event_cards::targeted::Context {
                        image: context.image,
                        catalog: &self.city.catalog.info,
                        products: &self.city.catalog.products,
                        commodities: &self.city.commodities.rows,
                        residents: &mut self.city.ledgers.residents,
                        tick: context.clock.timing[1],
                        accounting: context.accounting_enabled,
                        network: context.network_enabled,
                        network_override: context.network_override,
                    },
                    self.utilities,
                    |call, _| {
                        if let Event::EventCardTargeted(
                            crate::event_cards::targeted::Call::Message { from, to, building },
                        ) = call
                        {
                            self.message_context.produce(
                                [from as i32, to as i32, 0x2e, building, 0, 0, 0, -999],
                                &mut self.roster.manager,
                                |route, packet| {
                                    event(
                                        Event::PlayerMessageRoute {
                                            route,
                                            packet: *packet,
                                        },
                                        &*context.clock,
                                    )
                                },
                            )?;
                            Ok(())
                        } else {
                            event(call, &*context.clock)
                        }
                    },
                )?;
                if notice.enabled {
                    event(Event::EventCardNotice(notice), &*context.clock)?;
                }
                Ok(())
            }
            Command::OutcomeNotice { player, reason } => {
                // Retain the existing command observation, then execute its
                // original dialog and pause effects synchronously.
                event(
                    Event::OutcomeNotice {
                        player: *player,
                        reason: *reason,
                    },
                    &*context.clock,
                )?;
                crate::outcome_notice::present(
                    *player as u32,
                    *reason,
                    &crate::outcome_notice::World {
                        image: context.image,
                        players: &self.roster.manager.players,
                        current_reference: self.roster.status.current_player,
                        objective: &self.city.objective,
                    },
                    |call| {
                        if call == crate::outcome_notice::Call::Pause {
                            self.city.pause.pause(0, |timers, _| {
                                event(
                                    Event::Outcome(crate::outcome_notice::Call::Timers(timers)),
                                    &*context.clock,
                                )
                            })
                        } else {
                            event(Event::Outcome(call), &*context.clock)
                        }
                    },
                )
            }
            Command::Eliminate { slot } => eliminate(
                *slot,
                self.city,
                self.roster,
                &mut self.population.business_counts.age,
                self.message_context,
                &context.read(),
                |call| event(call, &*context.clock),
            ),
            Command::SellBusiness(command) => crate::business_sale::apply(
                command,
                &mut crate::business_sale::World {
                    businesses: &mut self.city.ledgers.businesses,
                    buildings: &mut self.city.buildings,
                    players: &mut self.city.ledgers.players,
                    catalog: &self.city.catalog.info,
                    products: &self.city.catalog.products,
                    commodities: &self.city.commodities.rows,
                    age: &mut self.population.business_counts.age,
                },
                &crate::business_sale::Controls {
                    current_player: self
                        .roster
                        .manager
                        .players
                        .iter()
                        .position(|player| player.reference == self.roster.status.current_player),
                    accounting: context.accounting_enabled,
                    ownership_tick: context.clock.timing[1],
                    timestamp: context.clock.timing[0],
                    hour: context.clock.hour() as i32,
                },
                context.image,
                |call, _| {
                    event(
                        match call {
                            crate::business_sale::Call::RefreshInterface => Event::RefreshInterface,
                            crate::business_sale::Call::AccountPosted(posting) => {
                                Event::AccountPosted(posting)
                            }
                            other => Event::BusinessSale(other),
                        },
                        &*context.clock,
                    )
                },
            ),
            Command::Takeover(command) => {
                let references: Vec<_> = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| p.reference)
                    .collect();
                let slots: Vec<_> = self
                    .roster
                    .manager
                    .players
                    .iter()
                    .map(|p| p.bytes[0x1bc])
                    .collect();
                crate::business_takeover::apply(
                    command,
                    &mut crate::business_takeover::World {
                        categories: &self.city.categories,
                        buildings: &mut self.city.buildings,
                        businesses: &mut self.city.ledgers.businesses,
                        players: &mut self.city.ledgers.players,
                        player_references: &references,
                        player_slots: &slots,
                        age: &mut self.population.business_counts.age,
                    },
                    &crate::business_takeover::Controls {
                        accounting: context.accounting_enabled,
                        ownership_tick: context.clock.timing[1],
                        network: self.message_context.network,
                        current_player: self
                            .roster
                            .manager
                            .players
                            .iter()
                            .position(|p| p.reference == self.roster.status.current_player),
                    },
                    context.image,
                    |call, world| match call {
                        crate::business_takeover::Call::Message(args) => {
                            for (account, player) in
                                world.players.iter().zip(&mut self.roster.manager.players)
                            {
                                account.write_runtime(&mut player.bytes)?;
                            }
                            player_message(
                                args,
                                &mut self.roster.manager,
                                self.message_context,
                                |call| event(call, &*context.clock),
                            )
                        }
                        crate::business_takeover::Call::RefreshInterface => {
                            event(Event::RefreshInterface, &*context.clock)
                        }
                        crate::business_takeover::Call::AccountPosted(posting) => {
                            event(Event::AccountPosted(posting), &*context.clock)
                        }
                        other => event(Event::BusinessTakeover(other), &*context.clock),
                    },
                )
            }
            Command::Convert(command) => {
                let quote = crate::business_conversion::quote(
                    command.building,
                    command.kind,
                    &self.city.categories,
                    &self.city.buildings,
                    &self.city.catalog.info,
                    context.image,
                )?;
                let posting = crate::business_conversion::debit(
                    command,
                    quote,
                    &mut self.city.ledgers.players,
                )?;
                event(Event::AccountPosted(posting), &*context.clock)?;
                open_with_tutorial(
                    &command.opening(),
                    self.city,
                    self.roster,
                    &mut self.population.business_counts,
                    self.utilities,
                    self.message_context,
                    context,
                    Some(geometry),
                    announcement,
                    event,
                )
                .map(|_| ())
            }
            Command::Demolish(command) => {
                demolish(command, self.city, self.roster, &context.read(), |call| {
                    event(call, &*context.clock)
                })
            }
            Command::Build(command) => {
                build(command, self.city, &context.read(), geometry, |call| {
                    event(call, &*context.clock)
                })
                .map(|_| ())
            }
            Command::Open(command) => open_with_tutorial(
                command,
                self.city,
                self.roster,
                &mut self.population.business_counts,
                self.utilities,
                self.message_context,
                context,
                Some(geometry),
                announcement,
                event,
            )
            .map(|_| ()),
            Command::Terms(command) => self
                .population
                .apply_business_terms(self.city, self.board, command),
        };
        // Even a host callback failure may follow original authoritative stores.
        let refresh = self.population.refresh_city(self.city, self.board);
        result.and(refresh)
    }
}

/// Original hourly447e20/48c450, without command16's later monopoly/view calls.
pub fn expire_leases(
    city: &mut City,
    roster: &mut Roster,
    messages: &mut crate::player_messages::Context,
    context: &Context<'_, '_>,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    use crate::{
        ai_messages::offers::auction,
        category_ownership::{Seam, World},
        player_manager::word,
    };
    roster.publish_accounts(city)?;
    let chains =
        crate::building_pool::category_chains(&city.categories, &city.buildings, 0x591318)?;
    let current = roster
        .manager
        .players
        .iter()
        .position(|p| p.reference == roster.status.current_player)
        .ok_or("lease current player unresolved")?;
    let mut controls = crate::category::Controls {
        current_owner_index: roster.manager.players[current].bytes[0x1bc],
        selected_category: Some(word(&city.auction.controller.record, 4)),
        selection_active: city.auction.controller.reference != 0
            && city.auction.controller.record[0x7d] != 0,
    };
    let mut world = World {
        categories: &mut city.categories,
        buildings: &mut city.buildings,
        businesses: &mut city.ledgers.businesses,
        players: &mut roster.manager.players,
        chains: &chains,
        normal_tick: context.clock.timing[1],
        category_tick: city.settings.lease_length() as u32,
        refresh_counter: city.ownership_refresh_counter,
        accounting_enabled: context.accounting_enabled,
        current_player: Some(current),
        notification_threshold: context.image.f32_at(0x525040)?,
    };
    let broadcast = word(&roster.manager.bytes, 12);
    let result =
        crate::category_lease::advance(
            &mut world,
            &mut controls,
            context.image,
            |call, world, controls| {
                match call.address {
                    0x48c450 => world.release(
                        call.arguments[0] as usize,
                        0.,
                        &mut |world, seam| match seam {
                            Seam::AiOwnershipLists { player } => {
                                crate::ai_ownership::maintain_player(
                                    &mut roster.ai_ownership,
                                    &mut world.players[player],
                                    world.categories,
                                    world.buildings,
                                    world.businesses,
                                    context.image,
                                    &city.catalog.info,
                                    |size| {
                                        crate::ai_runtime::allocate(&mut messages.arena.next, size)
                                    },
                                )
                            }
                            Seam::NativeUiCall {
                                address: 0x41f410, ..
                            } => event(Event::RefreshInterface),
                            Seam::AccountPosted(posting) => event(Event::AccountPosted(posting)),
                            other => event(Event::PropertyOwnership(other)),
                        },
                    )?,
                    0x464e30 => {
                        let state = &mut city.auction;
                        let mut kinds = world
                            .players
                            .iter()
                            .map(|p| word(&p.bytes, 0x154))
                            .collect();
                        let mut closure = auction::closure::World {
                            controller: &mut state.controller,
                            categories: world.categories,
                            presentation: &mut state.presentation,
                            player_kinds: &mut kinds,
                            network: messages.network,
                            command_prior: [0; 28],
                            row_text: &mut state.participant_ui,
                            bid_text: &mut state.bid_ui,
                        };
                        auction::closure::cleanup(&mut closure, &mut |call, closure| match call {
                            auction::closure::Call::Message(args) => {
                                crate::player_messages::produce_in(
                                    args,
                                    broadcast,
                                    &mut world.players,
                                    &mut messages.arena,
                                    crate::player_messages::Routing {
                                        current_player: messages.current_player,
                                        network: messages.network,
                                        prior: messages.prior,
                                    },
                                    |route, packet| {
                                        event(Event::PlayerMessageRoute {
                                            route,
                                            packet: *packet,
                                        })
                                    },
                                )
                                .map(|_| ())
                            }
                            other => {
                                if other == auction::closure::Call::CloseAnnouncement {
                                    closure.presentation.visible = false;
                                }
                                event(Event::AuctionClosure(other))
                            }
                        })?;
                    }
                    _ => event(Event::LeaseNotification(call))?,
                }
                controls.current_owner_index = world.players[current].bytes[0x1bc];
                controls.selected_category = Some(word(&city.auction.controller.record, 4));
                controls.selection_active = city.auction.controller.reference != 0
                    && city.auction.controller.record[0x7d] != 0;
                Ok(())
            },
        );
    city.ownership_refresh_counter = world.refresh_counter;
    roster.collect_accounts(city)?;
    result
}

/// Original command16 consumer: transfer first, then monopoly flags and UI.
#[allow(clippy::too_many_arguments)]
pub fn property_transfer(
    category: i16,
    owner: u8,
    amount: i32,
    city: &mut City,
    roster: &mut Roster,
    messages: &mut crate::player_messages::Context,
    context: &Context<'_, '_>,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    use crate::category_ownership::{Seam, World};
    roster.publish_accounts(city)?;
    let chains =
        crate::building_pool::category_chains(&city.categories, &city.buildings, 0x591318)?;
    let current = roster
        .manager
        .players
        .iter()
        .position(|p| p.reference == roster.status.current_player);
    let mut world = World {
        categories: &mut city.categories,
        buildings: &mut city.buildings,
        businesses: &mut city.ledgers.businesses,
        players: &mut roster.manager.players,
        chains: &chains,
        normal_tick: context.clock.timing[1],
        category_tick: city.settings.lease_length() as u32,
        refresh_counter: city.ownership_refresh_counter,
        accounting_enabled: context.accounting_enabled,
        current_player: current,
        notification_threshold: context.image.f32_at(0x525040)?,
    };
    let mut seam = |world: &mut World<'_>, call| match call {
        Seam::AiOwnershipLists { player } => crate::ai_ownership::maintain_player(
            &mut roster.ai_ownership,
            &mut world.players[player],
            world.categories,
            world.buildings,
            world.businesses,
            context.image,
            &city.catalog.info,
            |size| crate::ai_runtime::allocate(&mut messages.arena.next, size),
        ),
        Seam::NativeUiCall {
            address: 0x41f410, ..
        } => event(Event::RefreshInterface),
        Seam::AccountPosted(posting) => event(Event::AccountPosted(posting)),
        other => event(Event::PropertyOwnership(other)),
    };
    let result = if owner == 0 {
        world.release(category as usize, amount as f32, &mut seam)
    } else {
        world.apply(
            category as usize,
            usize::from(owner) - 1,
            amount as f32,
            &mut seam,
        )
    };
    city.ownership_refresh_counter = world.refresh_counter;
    roster.collect_accounts(city)?;
    result?;
    if owner != 0 {
        event(Event::PropertyTransferNotice {
            category,
            owner,
            amount,
        })?;
    }
    let identities: Vec<_> = roster.manager.players.iter().map(|p| p.reference).collect();
    for player in &mut roster.manager.players {
        if player.reference != 0 && player.bytes[0x10] == 1 {
            crate::category_ownership::update_monopoly(
                player,
                &identities,
                &city.categories,
                context.image,
            )?;
        }
    }
    event(Event::RefreshInterface)?;
    //450fb4 calls405260 with force0. Its announcement visibility gate precedes
    //the authoritative unlock, as well as both property-panel widget updates.
    if !crate::category_ownership::property_view(
        city.categories
            .get_mut(category as usize)
            .ok_or("property view category leaves pool")?,
        u8::from(city.auction.presentation.visible),
        0,
    ) {
        return Ok(());
    }
    event(Event::PropertyView(category))
}

pub fn build(
    command: &construction::Command,
    city: &mut City,
    context: &Context<'_, '_>,
    geometry: &mut impl crate::building_pool::GeometryBinding,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<usize, String> {
    construction::apply(
        command,
        &mut construction::World {
            buildings: &mut city.buildings,
            categories: &mut city.categories,
            pending: &mut city.pending_world,
            players: &mut city.ledgers.players,
        },
        &construction::Context {
            building_parameters: city.building_parameters,
            image: context.image,
            building_base: 0x591318,
            category_base: 0x617628,
            date: [
                context.clock.calendar[2] as u8,
                context.clock.calendar[1] as u8,
            ],
            ownership_timestamp: context.clock.timing[1],
        },
        geometry,
        |call, world| {
            if let construction::Event::Created(building) = call {
                crate::ai_space::refresh_building(
                    &mut city.category_footprints,
                    building,
                    world.buildings,
                    world.categories,
                )?;
                Ok(())
            } else if let construction::Event::AccountPosted(posting) = call {
                event(Event::AccountPosted(posting))
            } else {
                event(Event::Construction(call))
            }
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn open(
    command: &OpenBusiness,
    city: &mut City,
    roster: &mut Roster,
    counts: &mut crate::player_metrics::BusinessCounts,
    utilities: &mut economy::Utilities,
    messages: &mut crate::player_messages::Context,
    context: &Context<'_, '_>,
    event: impl FnMut(Event) -> Result<(), String>,
) -> Result<usize, String> {
    open_readonly(
        command, city, roster, counts, utilities, messages, context, event,
    )
}

#[allow(clippy::too_many_arguments)]
fn open_with_tutorial(
    command: &OpenBusiness,
    city: &mut City,
    roster: &mut Roster,
    counts: &mut crate::player_metrics::BusinessCounts,
    utilities: &mut economy::Utilities,
    messages: &mut crate::player_messages::Context,
    context: &mut CommandContext<'_, '_>,
    mut geometry: Option<&mut dyn crate::building_pool::GeometryBinding>,
    mut tutorial: Option<&mut dyn TutorialAnnouncement>,
    mut event: impl FnMut(Event, &crate::clock::Clock) -> Result<(), String>,
) -> Result<usize, String> {
    let mut command = command.clone();
    let mut age = counts.age;
    let result = crate::business_open::apply(
        &mut command,
        &mut age,
        &mut crate::business_open::Controls {
            network_enabled: context.network_enabled,
            network_override: context.network_override,
        },
        |call, command, _, _| match call.address {
            0x4634c0 => allocate(command, city, &context.read(), |call| {
                event(call, &*context.clock)
            })
            .map(|slot| slot.map_or(u32::MAX, |v| v as u32)),
            0x463560 => {
                bind_open(
                    command,
                    call.arguments[0] as usize,
                    city,
                    roster,
                    utilities,
                    &context.read(),
                    |call| event(call, &*context.clock),
                )?;
                Ok(0)
            }
            0x4e6960 => Err("original business allocation failed before binding".into()),
            0x48cab0 => {
                player_message(
                    call.arguments.map(|v| v as i32),
                    &mut roster.manager,
                    messages,
                    |call| event(call, &*context.clock),
                )?;
                Ok(0)
            }
            //420500 does not enqueue the current player's own construction.
            0x4504e0 => {
                use crate::player_manager::word;
                let current = roster
                    .manager
                    .players
                    .iter()
                    .find(|p| p.reference == roster.status.current_player)
                    .ok_or("open notification current player unresolved")?;
                if u32::from(current.bytes[0x1bc]) != command.owner.wrapping_sub(1) {
                    let building = city
                        .buildings
                        .get(command.building as usize)
                        .ok_or("construction notice building outside pool")?;
                    let business = word(building, 0x1c);
                    let business_kind = word(
                        city.ledgers
                            .businesses
                            .get(business as usize)
                            .ok_or("construction notice business outside pool")?,
                        4,
                    );
                    let category_ptr = word(building, 0x40);
                    let category = category_ptr
                        .checked_sub(0x617628)
                        .filter(|offset| offset % 380 == 0)
                        .ok_or("construction notice category pointer")?
                        / 380;
                    let category = word(
                        city.categories
                            .get(category as usize)
                            .ok_or("construction notice category outside pool")?,
                        0,
                    );
                    event(
                        Event::ConstructionNotice {
                            building: command.building,
                            owner: command.owner,
                            structure_kind: word(building, 0x24),
                            business,
                            business_kind,
                            category,
                        },
                        &*context.clock,
                    )?;
                }
                Ok(0)
            }
            0x41f410 => {
                event(Event::RefreshInterface, &*context.clock)?;
                Ok(0)
            }
            0x407520 => {
                city.buildings
                    .get_mut(command.building as usize)
                    .ok_or("open appearance building leaves pool")?[0x3d] = 0;
                event(Event::RefreshInterface, &*context.clock)?;
                Ok(0)
            }
            0x48dc40 if context.tutorial_state != 1 => Ok(0),
            0x48dc40 if call.count == 1 && call.arguments[0] == 99 && tutorial.is_some() => {
                let players: Vec<_> = roster
                    .manager
                    .players
                    .iter()
                    .map(|player| {
                        Some(business_binding::Player {
                            identity: player.reference,
                            kind: crate::player_manager::word(&player.bytes, 0x154),
                        })
                    })
                    .collect();
                tutorial.as_deref_mut().unwrap().control(
                    99,
                    &mut city.script_state(context.clock, &players),
                    geometry
                        .as_deref_mut()
                        .ok_or("tutorial completion geometry owner missing")?,
                    utilities,
                )?;
                Ok(0)
            }
            _ => {
                event(Event::OpenCall(call), &*context.clock)?;
                Ok(0)
            }
        },
    );
    counts.age = age;
    result.map(|slot| slot as usize)
}
fn open_readonly(
    command: &OpenBusiness,
    city: &mut City,
    roster: &mut Roster,
    counts: &mut crate::player_metrics::BusinessCounts,
    utilities: &mut economy::Utilities,
    messages: &mut crate::player_messages::Context,
    context: &Context<'_, '_>,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<usize, String> {
    let mut command = command.clone();
    let mut age = counts.age;
    let result = crate::business_open::apply(
        &mut command,
        &mut age,
        &mut crate::business_open::Controls {
            network_enabled: context.network_enabled,
            network_override: context.network_override,
        },
        |call, command, _, _| match call.address {
            0x4634c0 => allocate(command, city, context, &mut event)
                .map(|slot| slot.map_or(u32::MAX, |v| v as u32)),
            0x463560 => {
                bind_open(
                    command,
                    call.arguments[0] as usize,
                    city,
                    roster,
                    utilities,
                    context,
                    &mut event,
                )?;
                Ok(0)
            }
            0x4e6960 => Err("original business allocation failed before binding".into()),
            0x48cab0 => {
                player_message(
                    call.arguments.map(|v| v as i32),
                    &mut roster.manager,
                    messages,
                    &mut event,
                )?;
                Ok(0)
            }
            //420500 does not enqueue the current player's own construction.
            0x4504e0 => {
                use crate::player_manager::word;
                let current = roster
                    .manager
                    .players
                    .iter()
                    .find(|p| p.reference == roster.status.current_player)
                    .ok_or("open notification current player unresolved")?;
                if u32::from(current.bytes[0x1bc]) != command.owner.wrapping_sub(1) {
                    let building = city
                        .buildings
                        .get(command.building as usize)
                        .ok_or("construction notice building outside pool")?;
                    let business = word(building, 0x1c);
                    let business_kind = word(
                        city.ledgers
                            .businesses
                            .get(business as usize)
                            .ok_or("construction notice business outside pool")?,
                        4,
                    );
                    let category_ptr = word(building, 0x40);
                    let category = category_ptr
                        .checked_sub(0x617628)
                        .filter(|offset| offset % 380 == 0)
                        .ok_or("construction notice category pointer")?
                        / 380;
                    let category = word(
                        city.categories
                            .get(category as usize)
                            .ok_or("construction notice category outside pool")?,
                        0,
                    );
                    event(Event::ConstructionNotice {
                        building: command.building,
                        owner: command.owner,
                        structure_kind: word(building, 0x24),
                        business,
                        business_kind,
                        category,
                    })?;
                }
                Ok(0)
            }
            0x41f410 => {
                event(Event::RefreshInterface)?;
                Ok(0)
            }
            0x407520 => {
                city.buildings
                    .get_mut(command.building as usize)
                    .ok_or("open appearance building leaves pool")?[0x3d] = 0;
                event(Event::RefreshInterface)?;
                Ok(0)
            }
            0x48dc40 if context.tutorial_state != 1 => Ok(0),
            _ => {
                event(Event::OpenCall(call))?;
                Ok(0)
            }
        },
    );
    counts.age = age;
    result.map(|slot| slot as usize)
}
fn allocate(
    command: &OpenBusiness,
    city: &mut City,
    context: &Context<'_, '_>,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<Option<usize>, String> {
    struct Allocation<'a, F> {
        buildings: &'a [[u8; 0x394]],
        players: &'a mut [economy::PlayerAccount],
        accounting: bool,
        event: &'a mut F,
    }
    impl<F: FnMut(Event) -> Result<(), String>> business_pool::BusinessPoolHooks for Allocation<'_, F> {
        fn owner_transfer(
            &mut self,
            _: usize,
            record: &mut [u8; 544],
            owner: u32,
        ) -> Result<(), String> {
            let building = i16::from_le_bytes(record[8..10].try_into().unwrap()) as usize;
            let prefix = self
                .buildings
                .get(building)
                .ok_or("new business building leaves pool")?[..64]
                .try_into()
                .unwrap();
            economy::transfer_business_owner(
                record,
                prefix,
                owner as u8,
                self.accounting,
                self.players,
            )
        }
        fn notify(&mut self) -> Result<(), String> {
            (self.event)(Event::RefreshInterface)
        }
    }
    let business = business_pool::BusinessPool::new(
        city.ledgers
            .businesses
            .as_mut_slice()
            .try_into()
            .map_err(|_| "business pool must contain600 records")?,
    )
    .allocate(
        command.kind,
        command.owner,
        command.building as u16,
        &city.catalog.info,
        &mut Allocation {
            buildings: &city.buildings,
            players: &mut city.ledgers.players,
            accounting: context.accounting_enabled,
            event: &mut event,
        },
    )?;
    Ok(business)
}
fn bind_open(
    command: &OpenBusiness,
    business: usize,
    city: &mut City,
    roster: &mut Roster,
    utilities: &mut economy::Utilities,
    context: &Context<'_, '_>,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    let identities: Vec<_> = roster
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
    let event = std::cell::RefCell::new(&mut event);
    let accounts = std::cell::RefCell::new(&mut city.ledgers.players);
    let mut world = business_binding::World {
        buildings: &mut city.buildings,
        businesses: &mut city.ledgers.businesses,
        categories: &mut city.categories,
        category_base: 0x617628,
        catalog: &city.catalog.info,
        available: &mut city.available,
        availability_cursor: &mut city.availability_cursor,
        players: &identities,
        resident_count: city.ledgers.residents.active_count as usize,
    };
    utilities.owners = [2, 35, 39, 42].map(|i| world.categories[i][5]);
    let mut runtime = business_binding::Runtime {
        building_base: 0x591318,
        products: &city.catalog.products,
        commodities: &city.commodities.rows,
        residents: &mut city.ledgers.residents.nodes,
        timestamp: context.clock.timing[0],
    };
    runtime.bind_with_residents(
        &mut world,
        &business_binding::Inputs {
            business,
            building: command.building as usize,
            stock_argument: u32::from(command.stock_argument),
            hour: context.clock.hour() as i32,
            discount: context.image.f32_at(0x53be78)?,
            notifications: context.notifications,
        },
        context.image,
        &mut |call, world, residents| {
            match call {
                business_binding::Call::DebitBusiness { business, amount } => {
                    //4629d0(amount,1,0): rent accumulator only; payment was
                    // already posted by the construction command.
                    let row = &mut world.businesses[business];
                    let old = f32::from_le_bytes(row[0x28..0x2c].try_into().unwrap());
                    row[0x28..0x2c].copy_from_slice(
                        &((f64::from(old) + f64::from(amount)) as f32).to_le_bytes(),
                    );
                }
                business_binding::Call::CreditPlayer { identity, amount } => {
                    let slot = identities
                        .iter()
                        .position(|p| p.as_ref().is_some_and(|p| p.identity == identity))
                        .ok_or("business landlord unresolved")?;
                    accounts.borrow_mut()[slot].post(amount, 0x22)?;
                    event.borrow_mut()(Event::AccountPosted(economy::AccountPosting {
                        player: slot,
                        amount,
                    }))?;
                }
                business_binding::Call::ChargeUtilities { business } => {
                    world.assess_binding_utilities(business, utilities)?
                }
                business_binding::Call::PayUtilityOwners => {
                    for posting in
                        utilities.pay_owners(&mut accounts.borrow_mut(), context.utility_factor)?
                    {
                        event.borrow_mut()(Event::AccountPosted(posting))?;
                    }
                }
                business_binding::Call::DetachBusiness { business } => {
                    DetachRuntime {
                        accounts: &mut accounts.borrow_mut(),
                        demand: &mut city.ai_demand,
                        counts: &mut roster.category_counts,
                        products: &city.catalog.products,
                        commodities: &city.commodities.rows,
                        configuration: &city.configuration,
                        objective_rng: &mut city.creation.initialization_rng.objective,
                        memberships: city.commodities.memberships,
                    }
                    .apply(
                        business as i32,
                        world,
                        residents,
                        context,
                        &mut **event.borrow_mut(),
                    )?;
                }
                business_binding::Call::NotifyOwner {
                    category,
                    business_type,
                } => {
                    let index = category.wrapping_mul(45).wrapping_add(business_type) as usize;
                    let count = roster
                        .category_counts
                        .get_mut(index)
                        .ok_or("business count leaves table")?;
                    *count = count.wrapping_add(1);
                }
                business_binding::Call::NotifyBusiness { business } => {
                    city.ai_demand.business_changed(
                        business,
                        1,
                        world.businesses,
                        world.buildings,
                        world.catalog,
                        &city.catalog.products,
                    )?
                }
                other => event.borrow_mut()(Event::Business(other))?,
            }
            Ok(())
        },
        &mut |record, amount| {
            // Original462a00(amount,3): business balance first, then owner
            // cash/current stock ledger. The initializer already posted cost+30.
            let balance = f32::from_le_bytes(record[0x1c..0x20].try_into().unwrap());
            record[0x1c..0x20]
                .copy_from_slice(&((f64::from(balance) + f64::from(amount)) as f32).to_le_bytes());
            let owner = record[0xa] as i8;
            if owner != 0 {
                accounts
                    .borrow_mut()
                    .get_mut((i32::from(owner) - 1) as usize)
                    .ok_or("initial stock owner unresolved")?
                    .post(amount, 0x11)?;
                event.borrow_mut()(Event::AccountPosted(economy::AccountPosting {
                    player: (i32::from(owner) - 1) as usize,
                    amount,
                }))?;
            }
            Ok(())
        },
    )?;
    Ok(())
}

struct DetachRuntime<'a> {
    accounts: &'a mut [economy::PlayerAccount],
    demand: &'a mut crate::ai_demand::State,
    counts: &'a mut [u32; 52 * 45],
    products: &'a [[[u32; 2]; 6]; 45],
    commodities: &'a [[u8; 0x1c]; 32],
    configuration: &'a crate::resident::Configuration,
    objective_rng: &'a mut u32,
    memberships: [u32; 5],
}
impl DetachRuntime<'_> {
    fn apply(
        &mut self,
        business: i32,
        world: &mut business_binding::World<'_>,
        residents: &mut [crate::resident::Resident],
        context: &Context<'_, '_>,
        event: &mut impl FnMut(Event) -> Result<(), String>,
    ) -> Result<(), String> {
        use crate::business_detach::Call;
        crate::business_detach::apply(
            business,
            world,
            residents,
            context.notifications,
            context.image.f32_at(0x52508c)?,
            |call, world, residents| {
                match call {
                    Call::NotifyBusiness { business } => self.demand.business_changed(
                        business,
                        0,
                        world.businesses,
                        world.buildings,
                        world.catalog,
                        self.products,
                    )?,
                    Call::RemoveOwnerCount {
                        category,
                        business_type,
                    } => {
                        crate::ai_business::remove_category_count(
                            self.counts,
                            category,
                            business_type,
                        )?;
                    }
                    Call::EvictResident { resident } => {
                        let resident = &mut residents[resident];
                        // Building link is already clear, so45cd10 skips housing
                        // unlink; the native chain and bulk owner counters survive.
                        resident.relocate(-1, -1, None);
                        resident.refresh_daily_objective(
                            &self.configuration.objectives,
                            self.objective_rng,
                        );
                        resident.refresh_intent(
                            &self.configuration.timetables,
                            context.clock.hour() as usize,
                            self.memberships,
                        )?;
                    }
                    Call::RefreshCategory { category } => business_binding::refresh_category(
                        world,
                        category,
                        0x591318,
                        self.products,
                        self.commodities,
                    )?,
                    Call::TransferOwner { business, owner } => {
                        let index = i16::from_le_bytes(
                            world.businesses[business][8..10].try_into().unwrap(),
                        ) as usize;
                        let building = world
                            .buildings
                            .get(index)
                            .ok_or("detached owner building leaves pool")?;
                        economy::transfer_business_owner(
                            &mut world.businesses[business],
                            building[..64].try_into().unwrap(),
                            owner,
                            context.accounting_enabled,
                            self.accounts,
                        )?;
                    }
                    other => event(Event::Detach(other))?,
                }
                Ok(())
            },
        )
    }
}

pub fn demolish(
    command: &crate::demolition::Command,
    city: &mut City,
    roster: &mut Roster,
    context: &Context<'_, '_>,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    let identities: Vec<_> = roster
        .manager
        .players
        .iter()
        .map(|player| {
            Some(business_binding::Player {
                identity: player.reference,
                kind: u32::from_le_bytes(player.bytes[0x154..0x158].try_into().unwrap()),
            })
        })
        .collect();
    let mut world = crate::demolition::World {
        buildings: &mut city.buildings,
        businesses: &mut city.ledgers.businesses,
        categories: &mut city.categories,
        pending: &mut city.pending_world,
    };
    crate::demolition::apply(command, &mut world, |call, world| {
        match call {
            crate::demolition::Call::Post {
                owner,
                amount,
                ledger,
            } => {
                city.ledgers
                    .players
                    .get_mut(owner as usize - 1)
                    .ok_or("demolition owner unresolved")?
                    .post(amount, ledger)?;
                event(Event::AccountPosted(economy::AccountPosting {
                    player: owner as usize - 1,
                    amount,
                }))?;
            }
            crate::demolition::Call::Detach { business } => {
                let mut binding = business_binding::World {
                    buildings: world.buildings,
                    businesses: world.businesses,
                    categories: world.categories,
                    category_base: 0x617628,
                    catalog: &city.catalog.info,
                    available: &mut city.available,
                    availability_cursor: &mut city.availability_cursor,
                    players: &identities,
                    resident_count: city.ledgers.residents.active_count as usize,
                };
                DetachRuntime {
                    accounts: &mut city.ledgers.players,
                    demand: &mut city.ai_demand,
                    counts: &mut roster.category_counts,
                    products: &city.catalog.products,
                    commodities: &city.commodities.rows,
                    configuration: &city.configuration,
                    objective_rng: &mut city.creation.initialization_rng.objective,
                    memberships: city.commodities.memberships,
                }
                .apply(
                    business,
                    &mut binding,
                    &mut city.ledgers.residents.nodes,
                    context,
                    &mut event,
                )?;
            }
            crate::demolition::Call::Publish => {
                let category = world
                    .categories
                    .get(command.category as usize)
                    .ok_or("demolition notification category outside city")?;
                let building = world
                    .buildings
                    .get(command.building as usize)
                    .ok_or("demolition notification building outside pool")?;
                event(Event::DemolitionNotice {
                    owner: category[5] as i8,
                    building: command.building,
                    structure_kind: u32::from_le_bytes(building[0x24..0x28].try_into().unwrap()),
                    category: command.category,
                })?;
            }
            crate::demolition::Call::RefreshBuilding { building } => {
                // Native4075e0 clears +3c before its selected-interface branch.
                world
                    .buildings
                    .get_mut(building as usize)
                    .ok_or("demolition panel building outside pool")?[0x3c] = 0;
                event(Event::Demolition(call))?;
            }
            other => event(Event::Demolition(other))?,
        }
        Ok(())
    })
}

/// Presentation/resource boundaries around the authoritative pending-world pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingCall {
    World(crate::pending_world::Event),
    ReleaseBuilding { building: usize, reference: u32 },
    ReleaseEffect { building: usize, index: u32 },
    Refresh,
}
pub struct PendingRuntime<'a> {
    pub city: &'a mut City,
    pub roster: &'a Roster,
    pub commands: &'a mut Queue,
    /// Original590f60 and6175f8 logical building references.
    pub selected: &'a mut [u32; 2],
}
impl PendingRuntime<'_> {
    pub fn advance(
        &mut self,
        tick: u32,
        controls: &mut crate::pending_world::Controls,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        effect: impl FnMut(
            PendingCall,
            Option<&mut crate::pending_world::Building>,
        ) -> Result<u32, String>,
    ) -> Result<(), String> {
        self.advance_with_submission(tick, controls, geometry, effect, |command, commands| {
            commands.submit(command, tick, true, |_| {
                Err("wrapped construction completion requires reentrant command dispatch".into())
            })
        })
    }
    /// Connected callers collect authority-only completion requests here, then
    /// submit against the live target records after the transition pass releases
    /// its building borrow. Offline callers retain synchronous queue admission.
    pub fn advance_with_submission(
        &mut self,
        _tick: u32,
        controls: &mut crate::pending_world::Controls,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        effect: impl FnMut(
            PendingCall,
            Option<&mut crate::pending_world::Building>,
        ) -> Result<u32, String>,
        mut submit: impl FnMut(Command, &mut Queue) -> Result<(), String>,
    ) -> Result<(), String> {
        use crate::{building_pool, pending_world::Event};
        let effects = std::cell::RefCell::new(effect);
        let [selected, current] = self.selected;
        self.city.pending_world.advance(
            &mut self.city.buildings,
            controls,
            |event, _, buildings, _| match event {
                Event::Complete {
                    owner,
                    payload,
                    building_id,
                } => {
                    let player = self
                        .roster
                        .manager
                        .players
                        .get(owner.wrapping_sub(1) as usize)
                        .ok_or("construction completion owner leaves roster")?;
                    let command = OpenBusiness::from_completion(player, payload, building_id, 1)?;
                    submit(Command::Open(command), self.commands)?;
                    Ok(0)
                }
                Event::ModelHeight { building } => {
                    let handle =
                        u32::from_le_bytes(buildings[building][0x170..0x174].try_into().unwrap());
                    Ok(geometry.model_height(handle)?.to_bits())
                }
                Event::Remove {
                    category,
                    building_id,
                } => {
                    let category = self
                        .city
                        .categories
                        .get_mut(category as usize)
                        .ok_or("pending removal category leaves city")?;
                    building_pool::remove_category_building(
                        category,
                        buildings,
                        building_id,
                        0x591318,
                        &mut building_pool::RemovalGlobals {
                            global_590f60: selected,
                            global_6175f8: current,
                        },
                        geometry,
                        &mut building_pool::RemovalCallbacks {
                            release_resource: &mut |building, reference, record| {
                                effects.borrow_mut()(
                                    PendingCall::ReleaseBuilding {
                                        building,
                                        reference,
                                    },
                                    Some(record),
                                )
                                .map(|_| ())
                            },
                            release_model: &mut |building, index, record| {
                                effects.borrow_mut()(
                                    PendingCall::ReleaseEffect { building, index },
                                    Some(record),
                                )
                                .map(|_| ())
                            },
                            refresh: &mut || {
                                effects.borrow_mut()(PendingCall::Refresh, None).map(|_| ())
                            },
                        },
                    )?;
                    Ok(0)
                }
                other => {
                    let record = if let Event::Progress { building, .. } = other {
                        Some(&mut buildings[building])
                    } else {
                        None
                    };
                    effects.borrow_mut()(PendingCall::World(other), record)
                }
            },
        )
    }
}

/// Type10 consumer: release ownership while preserving player/asset pool slots.
pub fn eliminate(
    slot: usize,
    city: &mut City,
    roster: &mut Roster,
    business_age: &mut u8,
    messages: &mut crate::player_messages::Context,
    context: &Context<'_, '_>,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    use crate::player_elimination::{self, Call, Controls, Pools};
    roster.publish_accounts(city)?;
    let mut controls = Controls {
        network_enabled: context.network_enabled,
        current_player: roster.status.current_player,
        auction: city.auction.elimination_view(),
        refresh: [city.ownership_refresh_counter, *business_age],
    };
    let result = player_elimination::advance(
        slot,
        &mut roster.manager,
        Pools {
            categories: &mut city.categories,
            businesses: &mut city.ledgers.businesses,
            buildings: &mut city.buildings,
        },
        &mut controls,
        |call, manager, pools, _| {
            match call {
                Call::BusinessOwner { business } => {
                    let building =
                        i16::from_le_bytes(pools.businesses[business][8..10].try_into().unwrap())
                            as usize;
                    let prefix = pools
                        .buildings
                        .get(building)
                        .ok_or("elimination transfer building unresolved")?[..64]
                        .try_into()
                        .unwrap();
                    let mut accounts = manager
                        .players
                        .iter()
                        .map(|p| economy::PlayerAccount::from_runtime(&p.bytes))
                        .collect::<Result<Vec<_>, _>>()?;
                    let result = economy::transfer_business_owner(
                        &mut pools.businesses[business],
                        &prefix,
                        0,
                        context.accounting_enabled,
                        &mut accounts,
                    );
                    for (player, account) in manager.players.iter_mut().zip(accounts) {
                        account.write_runtime(&mut player.bytes)?;
                    }
                    result?;
                }
                Call::BuildingOwner { building } | Call::BuildingRefresh { building } => {
                    let row = &mut pools.buildings[building];
                    if matches!(call, Call::BuildingOwner { .. }) {
                        row[0x2d] = 0;
                        row[0x30..0x34].copy_from_slice(&context.clock.timing[1].to_le_bytes());
                    }
                    let word = |at| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
                    if word(0x4c) == 1 && (word(0x1c) == u32::MAX || matches!(word(0x24), 0 | 1)) {
                        event(Event::Elimination(call))?;
                    }
                }
                Call::Auction => {
                    use crate::ai_messages::offers::auction::closure;
                    let state = &mut city.auction;
                    let mut kinds = manager
                        .players
                        .iter()
                        .map(|p| crate::player_manager::word(&p.bytes, 0x154))
                        .collect();
                    let mut world = closure::World {
                        controller: &mut state.controller,
                        categories: pools.categories,
                        presentation: &mut state.presentation,
                        player_kinds: &mut kinds,
                        network: messages.network,
                        command_prior: [0; 28],
                        row_text: &mut state.participant_ui,
                        bid_text: &mut state.bid_ui,
                    };
                    closure::cleanup(&mut world, &mut |call, world| match call {
                        closure::Call::Message(args) => messages
                            .produce(args, manager, |route, packet| {
                                event(Event::PlayerMessageRoute {
                                    route,
                                    packet: *packet,
                                })
                            })
                            .map(|_| ()),
                        other => {
                            if other == closure::Call::CloseAnnouncement {
                                world.presentation.visible = false;
                            }
                            event(Event::AuctionClosure(other))
                        }
                    })?;
                }
                other => event(Event::Elimination(other))?,
            }
            Ok(())
        },
    );
    // Auction cleanup retains the original controller allocation.
    city.ownership_refresh_counter = controls.refresh[0];
    *business_age = controls.refresh[1];
    roster.collect_accounts(city)?;
    result
}

/// Bind the result controller's producers to the same local25-tick queue.
/// Per-player inbox packets remain synchronous host boundaries.
pub fn queue_outcome(
    call: crate::objective_tick::Event,
    queue: &mut Queue,
    tick: u32,
    manager: &mut crate::player_manager::Manager,
    messages: &mut crate::player_messages::Context,
    event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    use crate::objective_tick::Event as Outcome;
    match call {
        Outcome::Outcome { winner, reason } => queue.submit(
            Command::OutcomeNotice {
                player: winner,
                reason,
            },
            tick,
            true,
            |_| Err("outcome delay wrapped into immediate dispatch".into()),
        ),
        Outcome::RemovePlayer(slot) => {
            queue.submit(Command::Eliminate { slot }, tick, true, |_| {
                Err("elimination delay wrapped into immediate dispatch".into())
            })
        }
        Outcome::RemovalCommand(slot) => player_message(
            [slot as i32, -1, 0x2d, 0, 0, 0, 0, -999],
            manager,
            messages,
            event,
        ),
        Outcome::Metric { .. } => Err("metric query is not an outcome command".into()),
    }
}
/// Retain local messages in the actual player inbox; expose only external routes.
pub fn player_message(
    args: [i32; 8],
    manager: &mut crate::player_manager::Manager,
    messages: &mut crate::player_messages::Context,
    mut event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    messages
        .produce(args, manager, |route, packet| {
            event(Event::PlayerMessageRoute {
                route,
                packet: *packet,
            })
        })
        .map(|_| ())
}

/// Bind the midnight warning/recovery/bankruptcy pass to retained commands.
pub fn queue_cash_status(
    call: crate::player_manager::CashEvent,
    queue: &mut Queue,
    tick: u32,
    manager: &mut crate::player_manager::Manager,
    messages: &mut crate::player_messages::Context,
    event: impl FnMut(Event) -> Result<(), String>,
) -> Result<(), String> {
    use crate::{objective_tick::Event as Outcome, player_manager::CashEvent};
    let outcome = match call {
        CashEvent::Command(args) => return player_message(args, manager, messages, event),
        CashEvent::RemovalRequest(slot) => Outcome::Outcome {
            winner: slot as i32,
            reason: 0,
        },
        CashEvent::RemovePlayer(slot) => Outcome::RemovePlayer(slot),
    };
    queue_outcome(outcome, queue, tick, manager, messages, event)
}
