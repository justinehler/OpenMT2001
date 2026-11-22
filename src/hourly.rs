//! Original0047f4b0 hourly world dispatch. Called before the clock rolls24 to0.

#[derive(Clone, Copy)]
pub struct Controls {
    /// Original008b0700 suppresses the entire pass, including its query.
    pub suppressed: bool,
    pub world_mode: u32,
    pub before_six_sound: u32,
    pub network_enabled: bool,
    pub network_override: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Query,
    Script {
        name: u32,
        argument: u32,
    },
    StopSound(u32),
    Businesses,
    Categories,
    CloseBusinessDay,
    RotatePlayerLedgers,
    DailyBusinesses,
    FinishPlayerDay,
    MorningResidents,
    EveningResidents,
    ///0041cd70 on the current object's +21c callback target.
    CurrentPlayerUpdate,
    ///0041cd70 on this manager slot's +21c callback target.
    PlayerUpdate(usize),
    Helper(u32),
}

/// Hourly finance borrows the same category topology used by shopping and the
/// same building prefixes changed by expiry. It does not retain daily input
/// snapshots: EndOfDay script changes are visible before463d40 runs.
pub struct Finance<'a> {
    pub topology: &'a mut crate::shopping::Topology,
    pub buildings: &'a mut [[u8; 64]],
    pub shopping: &'a crate::shopping::Settings,
    pub business: &'a crate::economy::BusinessDaySettings,
    pub utilities: &'a mut crate::economy::Utilities,
    pub properties: &'a mut crate::economy::PropertyIncome,
    pub income: crate::economy::OwnerIncomeSettings,
}

/// Runtime expiry inputs omitted from the saved building prefix. Render kinds
/// must come from successful scene preparation (original488f9c), not byte3f.
pub struct CategoryContext<'a> {
    pub render_kinds: &'a [u32],
    pub controls: crate::category::Controls,
    pub normal_tick: u32,
    pub accounting_enabled: bool,
    pub refresh_counter: u8,
}

/// Resident scheduling borrows shared accounts/businesses from Finance's
/// ledger caller. RNG streams remain explicit until their load provenance is
/// reconstructed; advancing an hour does not fabricate a fresh random seed.
pub struct Residents<'a> {
    pub housing: &'a crate::housing::Topology,
    pub housing_settings: &'a crate::housing::Settings,
    pub welfare: &'a crate::resident::Welfare,
    pub objectives: &'a crate::resident::DailyObjectives,
    pub objective_rng: &'a mut u32,
    pub housing_countdown: &'a mut crate::housing::Countdown,
    pub available: &'a mut Vec<usize>,
    pub market: &'a mut crate::market::Cache,
}

impl Residents<'_> {
    pub fn advance(
        &mut self,
        finance: &mut Finance<'_>,
        clock: &mut crate::clock::Clock,
        controls: &mut Controls,
        ledgers: &mut crate::economy::Ledgers,
        mut callback: impl FnMut(
            Event,
            &mut crate::clock::Clock,
            &mut Controls,
            &mut crate::economy::Ledgers,
            &mut Finance<'_>,
            &mut Self,
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        finance.advance(
            clock,
            controls,
            ledgers,
            |event, clock, controls, ledgers, finance| {
                match event {
                    Event::MorningResidents => ledgers.residents.update_daily(
                        self.welfare,
                        &finance.shopping.preferences,
                        &finance.shopping.commodities,
                        self.objectives,
                        self.objective_rng,
                    )?,
                    Event::EveningResidents => {
                        let mut average_rent = |businesses: &[[u8; 544]]| {
                            self.market.residential_rent(&crate::market::Inputs {
                                businesses,
                                buildings: finance.buildings,
                                catalogs: &finance.shopping.catalogs,
                                commodities: &finance.shopping.commodities,
                            })
                        };
                        let mut world = crate::housing::World {
                            demand: None,
                            buildings: &self.housing.buildings,
                            categories: &self.housing.categories,
                            category_values: &finance.topology.category_factors,
                            businesses: &mut ledgers.businesses,
                            available: self.available,
                            players: &mut ledgers.players,
                            welfare: self.welfare,
                            settings: self.housing_settings,
                            average_rent: &mut average_rent,
                        };
                        crate::housing::update_evening(
                            &mut ledgers.residents,
                            &mut world,
                            self.objectives,
                            self.objective_rng,
                            self.housing_countdown,
                        )?;
                    }
                    _ => return callback(event, clock, controls, ledgers, finance, self),
                }
                Ok(false)
            },
        )
    }
}

impl Finance<'_> {
    /// Bind Event::Categories to expiry on the very same category/building/
    /// ledger records used by the subsequent daily finance and shopping passes.
    /// Appearance, notification and AI callbacks retain their original order.
    pub fn expire_categories(
        &mut self,
        ledgers: &mut crate::economy::Ledgers,
        context: &mut CategoryContext<'_>,
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
        let chains = std::array::from_fn(|category| {
            self.topology.nodes[category]
                .iter()
                .map(|node| node.building as usize)
                .collect()
        });
        let mut expiry = crate::category::Expiry {
            buildings: self.buildings,
            render_kinds: context.render_kinds,
            chains: &chains,
            ledgers,
            normal_tick: context.normal_tick,
            accounting_enabled: context.accounting_enabled,
            refresh_counter: context.refresh_counter,
        };
        let result = expiry.advance_hour(
            &mut self.topology.categories,
            &mut context.controls,
            callout,
            event,
        );
        context.normal_tick = expiry.normal_tick;
        context.accounting_enabled = expiry.accounting_enabled;
        context.refresh_counter = expiry.refresh_counter;
        result
    }

    pub fn advance(
        &mut self,
        clock: &mut crate::clock::Clock,
        controls: &mut Controls,
        ledgers: &mut crate::economy::Ledgers,
        mut callback: impl FnMut(
            Event,
            &mut crate::clock::Clock,
            &mut Controls,
            &mut crate::economy::Ledgers,
            &mut Self,
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        let counts = std::array::from_fn(|i| self.shopping.catalogs.commodities[i].len() as u8);
        advance_ledgers(
            clock,
            controls,
            ledgers,
            &counts,
            |event, clock, controls, ledgers| {
                match event {
                    Event::DailyBusinesses => {
                        let owners = self.topology.categories.map(|category| category.owner);
                        self.utilities.owners = [2, 35, 39, 42].map(|i| owners[i]);
                        let parameters = crate::economy::BusinessDayInputs {
                            buildings: self.buildings,
                            building_categories: &self.topology.building_categories,
                            category_owners: &owners,
                            catalogs: &self.shopping.catalogs,
                            unit_prices: &self.shopping.unit_costs,
                            settings: self.business,
                        }
                        .parameters(&ledgers.businesses)?;
                        crate::economy::update_daily_businesses(
                            &mut ledgers.businesses,
                            &parameters,
                            &mut ledgers.residents,
                            &mut ledgers.players,
                            self.utilities,
                        )?;
                    }
                    Event::Helper(0x48e070) => {
                        let owners = self.topology.categories.map(|category| category.owner);
                        self.utilities.owners = [2, 35, 39, 42].map(|i| owners[i]);
                        self.properties.owners = [4, 17, 43, 44].map(|i| owners[i]);
                        self.income.pay_owners(
                            self.utilities,
                            self.properties,
                            &mut ledgers.players,
                        )?;
                    }
                    _ => return callback(event, clock, controls, ledgers, self),
                }
                Ok(false)
            },
        )
    }
}

/// Bind the recovered hourly business state, midnight history/ledger rotation
/// and morning modifier reset to one shared ledger set. Remaining events must
/// be handled by the caller, including Categories and DailyBusinesses with
/// their additional world inputs. No host I/O is performed here.
pub fn advance_ledgers(
    clock: &mut crate::clock::Clock,
    controls: &mut Controls,
    ledgers: &mut crate::economy::Ledgers,
    catalog_counts: &[u8; 45],
    mut callback: impl FnMut(
        Event,
        &mut crate::clock::Clock,
        &mut Controls,
        &mut crate::economy::Ledgers,
    ) -> Result<bool, String>,
) -> Result<(), String> {
    advance(
        clock,
        controls,
        ledgers,
        |event, clock, controls, ledgers| {
            match event {
                Event::Businesses => ledgers.update_hourly_businesses(clock.hour()),
                Event::CloseBusinessDay => ledgers.close_business_day(catalog_counts)?,
                Event::RotatePlayerLedgers => {
                    for player in &mut ledgers.players {
                        player.rotate_ledger();
                    }
                }
                Event::FinishPlayerDay => {
                    for player in &mut ledgers.players {
                        player.finish_daily_update();
                    }
                }
                _ => return callback(event, clock, controls, ledgers),
            }
            Ok(false)
        },
    )
}

/// Keep callbacks synchronous: scripts and world updates may change the hour,
/// player list, controller kinds or network gates before subsequent reads.
/// Only Query's returned boolean is consumed. Unknown helper bodies remain
/// explicit host/world boundaries, not simulated successes.
pub fn advance(
    clock: &mut crate::clock::Clock,
    controls: &mut Controls,
    ledgers: &mut crate::economy::Ledgers,
    callback: impl FnMut(
        Event,
        &mut crate::clock::Clock,
        &mut Controls,
        &mut crate::economy::Ledgers,
    ) -> Result<bool, String>,
) -> Result<(), String> {
    advance_world(clock, controls, ledgers, callback)
}

/// Access only the live roster fields used by the dispatcher. A complete city
/// can implement this so callbacks borrow its scripts and accounts together,
/// without moving the ledgers out of the authoritative world.
pub trait World {
    fn players(&self) -> &[crate::economy::PlayerAccount];
}
impl World for crate::economy::Ledgers {
    fn players(&self) -> &[crate::economy::PlayerAccount] {
        &self.players
    }
}

pub fn advance_world<W: World>(
    clock: &mut crate::clock::Clock,
    controls: &mut Controls,
    ledgers: &mut W,
    mut callback: impl FnMut(
        Event,
        &mut crate::clock::Clock,
        &mut Controls,
        &mut W,
    ) -> Result<bool, String>,
) -> Result<(), String> {
    if controls.suppressed || callback(Event::Query, clock, controls, ledgers)? {
        return Ok(());
    }
    callback(
        Event::Script {
            name: 0x5414fc,
            argument: clock.hour(),
        },
        clock,
        controls,
        ledgers,
    )?;
    if clock.hour() == 18 {
        callback(Event::Helper(0x4899e0), clock, controls, ledgers)?;
    }
    callback(Event::Helper(0x4895b0), clock, controls, ledgers)?;
    if controls.world_mode != 3 {
        callback(Event::Helper(0x44f2a0), clock, controls, ledgers)?;
    }
    callback(Event::Businesses, clock, controls, ledgers)?;
    callback(Event::Categories, clock, controls, ledgers)?;
    if clock.hour() == 24 {
        callback(Event::CloseBusinessDay, clock, controls, ledgers)?;
        callback(Event::RotatePlayerLedgers, clock, controls, ledgers)?;
    }
    if clock.hour() == 6 {
        if controls.before_six_sound != 0 {
            callback(
                Event::StopSound(controls.before_six_sound),
                clock,
                controls,
                ledgers,
            )?;
            controls.before_six_sound = 0;
        }
        callback(
            Event::Script {
                name: 0x5414f0,
                argument: 0,
            },
            clock,
            controls,
            ledgers,
        )?;
        callback(Event::DailyBusinesses, clock, controls, ledgers)?;
        callback(Event::FinishPlayerDay, clock, controls, ledgers)?;
        callback(Event::Helper(0x48e070), clock, controls, ledgers)?;
    }
    if clock.hour() == 10 {
        callback(
            Event::Script {
                name: 0x5414e4,
                argument: clock.calendar[4],
            },
            clock,
            controls,
            ledgers,
        )?;
    }
    if clock.hour() == 5 {
        callback(Event::MorningResidents, clock, controls, ledgers)?;
    }
    if clock.hour() == 17 {
        callback(Event::EveningResidents, clock, controls, ledgers)?;
    }
    callback(Event::CurrentPlayerUpdate, clock, controls, ledgers)?;
    if !controls.network_enabled || controls.network_override {
        let mut index = 0;
        while index < ledgers.players().len() {
            if ledgers.players()[index].controller_kind == 1 {
                callback(Event::PlayerUpdate(index), clock, controls, ledgers)?;
            }
            index += 1;
        }
    }
    callback(Event::Helper(0x470b80), clock, controls, ledgers)?;
    callback(Event::Helper(0x4bb500), clock, controls, ledgers)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original hourly dispatcher observations"]
    fn matches_native_dispatcher() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_HOURLY_DISPATCH").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTDISP02");
        let mut at = 12;
        for case in 0..word(8) {
            let config: [u32; 16] = std::array::from_fn(|i| word(at + i * 4));
            at += 64;
            let kinds: [u32; 6] = std::array::from_fn(|i| word(at + i * 4));
            at += 24;
            let initial: [u32; 8] = std::array::from_fn(|i| word(at + i * 4));
            at += 32;
            let expected: [u32; 8] = std::array::from_fn(|i| word(at + i * 4));
            at += 32;
            let mut clock = crate::clock::Clock {
                calendar: [initial[0], 0, 0, 0, initial[1]],
                timing: [0; 6],
            };
            let mut controls = Controls {
                before_six_sound: initial[2],
                world_mode: initial[4],
                suppressed: initial[5] != 0,
                network_enabled: initial[6] != 0,
                network_override: initial[7] != 0,
            };
            let player = |index: usize| {
                let mut raw = [0; 440];
                raw[0x150..0x154].copy_from_slice(&kinds[index].to_le_bytes());
                crate::economy::PlayerAccount::from_saved(&crate::save::SavedPlayer {
                    metadata: &[0; 48],
                    bytes: &raw,
                })
                .unwrap()
            };
            let mut ledgers = crate::economy::Ledgers {
                businesses: Vec::new(),
                players: (0..initial[3] as usize).map(player).collect(),
                residents: crate::economy::ResidentAccounts {
                    active_count: 0,
                    nodes: Vec::new(),
                    accounts: Vec::new(),
                },
            };
            let snapshot = |clock: &crate::clock::Clock,
                            controls: &Controls,
                            ledgers: &crate::economy::Ledgers| {
                [
                    clock.hour(),
                    clock.calendar[4],
                    controls.before_six_sound,
                    ledgers.players.len() as u32,
                    controls.world_mode,
                    u32::from(controls.suppressed),
                    u32::from(controls.network_enabled),
                    u32::from(controls.network_override),
                ]
            };
            let mut trace = Vec::new();
            let mut scripts = 0;
            let mut controllers = 0;
            advance(
                &mut clock,
                &mut controls,
                &mut ledgers,
                |event, clock, controls, ledgers| {
                    let (target, controller, arguments) = match event {
                        Event::Query => (0x48dbf0, 0, [0; 2]),
                        Event::Script { name, argument } => (0x4d5fb0, 0, [name, argument]),
                        Event::StopSound(sound) => (0x4af3a0, 0, [sound, 0]),
                        Event::Businesses => (0x464100, 0, [0; 2]),
                        Event::Categories => (0x447e20, 0, [0; 2]),
                        Event::CloseBusinessDay => (0x463d00, 0, [0; 2]),
                        Event::RotatePlayerLedgers => (0x48c9b0, 0, [0; 2]),
                        Event::DailyBusinesses => (0x463d40, 0, [0; 2]),
                        Event::FinishPlayerDay => (0x48ca10, 0, [0; 2]),
                        Event::MorningResidents => (0x45bd80, 0, [0; 2]),
                        Event::EveningResidents => (0x45bdd0, 0, [0; 2]),
                        Event::CurrentPlayerUpdate => (0x41cd70, config[14], [0; 2]),
                        Event::PlayerUpdate(index) => (0x41cd70, 0x22000000 + index as u32, [0; 2]),
                        Event::Helper(address) => (address, 0, [0; 2]),
                    };
                    trace.push((
                        target,
                        controller,
                        snapshot(clock, controls, ledgers),
                        arguments,
                    ));
                    if target == 0x4d5fb0 {
                        if scripts == config[12] {
                            clock.calendar[0] = config[9];
                        }
                        scripts += 1;
                    }
                    if target == 0x41cd70 {
                        if controllers == config[13] {
                            ledgers
                                .players
                                .resize_with(config[10] as usize, || player(0));
                            // Restore explicit kinds for newly exposed manager slots.
                            for (index, value) in ledgers.players.iter_mut().enumerate() {
                                value.controller_kind = kinds[index];
                            }
                        }
                        controllers += 1;
                    }
                    if target == 0x4af3a0 && config[11] != u32::MAX {
                        controls.before_six_sound = config[11];
                    }
                    Ok(event == Event::Query && config[8] & 255 != 0)
                },
            )
            .unwrap();
            assert_eq!(
                snapshot(&clock, &controls, &ledgers),
                expected,
                "case{case} globals"
            );
            assert_eq!(trace.len(), word(at) as usize, "case{case} call count");
            at += 4;
            for (target, controller, before, arguments) in trace {
                assert_eq!(target, word(at), "case{case} target");
                assert_eq!(controller, word(at + 8), "case{case} controller target");
                assert_eq!(
                    before,
                    std::array::from_fn(|i| word(at + 12 + i * 4)),
                    "case{case} pre-call globals"
                );
                assert_eq!(
                    arguments,
                    [word(at + 44), word(at + 48)],
                    "case{case} arguments"
                );
                at += 52;
            }
        }
        assert_eq!(at, bytes.len());
    }
}
