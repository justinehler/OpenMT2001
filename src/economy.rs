//! Recovered business calculations. Scheduling and state loading are separate.

#[derive(Clone, Copy)]
pub enum ScriptMoneyOperation {
    Set,
    Increase,
    Decrease,
}

/// Original4d6f10/4d6f40/4d6f80. Both increase and decrease add their signed
/// argument; DEFAULT's decrease event supplies -100. No ledger entry is posted.
pub fn script_money(cash: f32, argument: f64, operation: ScriptMoneyOperation) -> f32 {
    let amount = f64::from(ftol_low_word(argument));
    match operation {
        ScriptMoneyOperation::Set => amount as f32,
        ScriptMoneyOperation::Increase | ScriptMoneyOperation::Decrease => {
            (f64::from(cash) + amount) as f32
        }
    }
}

/// Presentation notification for an account mutation that has already happened.
/// `player` is a zero-based player slot; observers must not apply the amount again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccountPosting {
    pub player: usize,
    pub amount: f32,
}

/// Cash and the two native 38-category ledger periods at player +0x14.
/// Host UI/sound effects from 0048a5a0 are handled separately.
#[derive(Clone, Debug)]
pub struct PlayerAccount {
    pub cash: f32,
    /// Original linked player +0x148; event-controlled replenishment modifier.
    pub replenishment_modifier: u32,
    /// Original player +1ac/+1b4; retail adds quantity*20 / quantity.
    pub retail_counters: [i32; 2],
    /// Original player +1b0; residential unlink subtracts 50.
    pub housing_counter: i32,
    /// Original player+178..1ac, indexed by category.
    pub owned_categories: [u8; 52],
    /// Original player+154; value1 requests AI ownership-list maintenance.
    pub controller_kind: u32,
    ledgers: [[f32; 38]; 2],
    period: usize,
}

impl PlayerAccount {
    pub fn apply_script_money(&mut self, argument: f64, operation: ScriptMoneyOperation) {
        self.cash = script_money(self.cash, argument, operation);
    }
    pub fn from_saved(player: &crate::save::SavedPlayer<'_>) -> Result<Self, String> {
        Self::from_prefix(player.bytes)
    }

    /// Decode the common account fields of a constructed runtime player.
    pub fn from_runtime(bytes: &[u8]) -> Result<Self, String> {
        Self::from_prefix(bytes.get(4..).ok_or("truncated runtime player")?)
    }

    /// Publish accounting mutations to the existing runtime player. Preserve
    /// its vtable, activity, controller, auxiliary references and all other data.
    /// Call before a script/controller reads the manager's player bytes again.
    pub fn write_runtime(&self, bytes: &mut [u8]) -> Result<(), String> {
        let bytes = bytes
            .get_mut(..0x1b8)
            .ok_or("truncated runtime player account")?;
        bytes[4..8].copy_from_slice(&self.cash.to_le_bytes());
        for (period, bank) in self.ledgers.iter().enumerate() {
            for (event, value) in bank.iter().enumerate() {
                let at = 0x14 + (period * 38 + event) * 4;
                bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes[0x144] = self.period as u8;
        bytes[0x148..0x14c].copy_from_slice(&self.replenishment_modifier.to_le_bytes());
        bytes[0x178..0x1ac].copy_from_slice(&self.owned_categories);
        bytes[0x1ac..0x1b0].copy_from_slice(&self.retail_counters[0].to_le_bytes());
        bytes[0x1b0..0x1b4].copy_from_slice(&self.housing_counter.to_le_bytes());
        bytes[0x1b4..0x1b8].copy_from_slice(&self.retail_counters[1].to_le_bytes());
        Ok(())
    }

    fn from_prefix(bytes: &[u8]) -> Result<Self, String> {
        let bytes = bytes
            .get(..0x1b4)
            .ok_or("truncated player account prefix")?;
        let period = usize::from(bytes[0x140]);
        if period >= 2 {
            return Err("player ledger period exceeds its two arrays".into());
        }
        Ok(Self {
            cash: f32::from_le_bytes(bytes[..4].try_into().unwrap()),
            owned_categories: bytes[0x174..0x1a8].try_into().unwrap(),
            controller_kind: u32::from_le_bytes(bytes[0x150..0x154].try_into().unwrap()),
            housing_counter: i32::from_le_bytes(bytes[0x1ac..0x1b0].try_into().unwrap()),
            retail_counters: [0x1a8, 0x1b0]
                .map(|at| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())),
            replenishment_modifier: u32::from_le_bytes(bytes[0x144..0x148].try_into().unwrap()),
            ledgers: std::array::from_fn(|p| {
                std::array::from_fn(|event| {
                    let at = 0x10 + (p * 38 + event) * 4;
                    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
                })
            }),
            period,
        })
    }

    pub fn ledger(&self, event: usize) -> Option<f32> {
        self.ledgers[self.period].get(event).copied()
    }

    /// 0048c9b0 at phase 24: retain the old period and clear the new one.
    /// Cash is independent of the ledger period.
    pub fn rotate_ledger(&mut self) {
        self.period = usize::from(self.period == 0);
        self.ledgers[self.period] = [0.; 38];
    }

    /// 0048ca10 resets this field after the phase-6 business update.
    pub fn finish_daily_update(&mut self) {
        self.replenishment_modifier = 0;
    }

    /// 0048a4a0 records an event without transferring cash.
    pub fn record(&mut self, amount: f32, event: usize) -> Result<(), String> {
        let entry = self.ledgers[self.period]
            .get_mut(event)
            .ok_or("player event exceeds native ledger categories")?;
        *entry = (f64::from(amount) + f64::from(*entry)) as f32;
        Ok(())
    }

    /// 0048a5a0 stores the event first, then adds the same rounded amount
    /// independently to cash. Negative amounts are ordinary debits.
    pub fn post(&mut self, amount: f32, event: usize) -> Result<(), String> {
        self.record(amount, event)?;
        self.cash = (f64::from(amount) + f64::from(self.cash)) as f32;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RestockLine {
    pub stock: u16,
    pub target: i16,
    pub unit_cost: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BusinessAccount {
    /// Runtime business +0x30.
    pub cost_balance: f32,
    /// Runtime business +0x1c, updated by 00462a00.
    pub balance: f32,
}

/// Resident records and spending accounts in original slot order. Loaded saves
/// contain occupied residents only; fresh pools retain all2000 runtime slots.
pub struct ResidentAccounts {
    /// Signed-positive prefix length used by the original resident passes.
    pub active_count: u32,
    pub nodes: Vec<crate::resident::Resident>,
    pub accounts: Vec<crate::resident::SpendingAccount>,
}

/// One mutable set of saved ledgers shared by shopping, housing and daily
/// business updates. Auxiliary/model load fixups remain a separate step.
pub struct Ledgers {
    pub businesses: Vec<[u8; 544]>,
    pub residents: ResidentAccounts,
    pub players: Vec<PlayerAccount>,
}

impl Ledgers {
    /// The load path destroys the old six-slot manager, then recreates players
    /// in save order. Kind0 has the short player class; kinds1/2 construct the
    /// extended class and reset its runtime AI records before the prefix copy.
    /// These fields are not serialized. The returned vector's index is native
    /// player+1bc; saved bytes stop immediately before that runtime slot byte.
    pub fn initialize_ai_ownership(
        &self,
        image: &crate::pe::PeImage<'_>,
    ) -> Result<Vec<Option<crate::ai_ownership::State>>, String> {
        if self.players.len() > 6 {
            return Err("saved players exceed the original six-slot manager".into());
        }
        self.players
            .iter()
            .map(|player| match player.controller_kind {
                0 => Ok(None),
                1 | 2 => crate::ai_ownership::State::from_v14(image).map(Some),
                _ => Err("saved player kind has no original constructor".into()),
            })
            .collect()
    }

    pub fn from_saved(save: &crate::save::SaveFile<'_>) -> Result<Self, String> {
        Ok(Self {
            businesses: save.economy()?.businesses().map(|b| *b.bytes).collect(),
            residents: ResidentAccounts::from_saved(&save.residents()?),
            players: save
                .players()?
                .iter()
                .map(PlayerAccount::from_saved)
                .collect::<Result<_, _>>()?,
        })
    }

    /// Original00464100, called by the hourly dispatcher before daily finance.
    pub fn update_hourly_businesses(&mut self, hour: u32) {
        update_hourly_businesses(&mut self.businesses, hour);
    }

    ///00463d00: rotate each active business, then reset its +6c state from
    /// the discriminator. The discriminator itself is not changed.
    pub fn close_business_day(&mut self, catalog_counts: &[u8; 45]) -> Result<(), String> {
        if self.businesses.len() != 600 {
            return Err("business day close requires the original 600-slot table".into());
        }
        for record in &mut self.businesses {
            if record[0x21c] == 0 {
                continue;
            }
            let kind = u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize;
            let count = *catalog_counts
                .get(kind)
                .ok_or("business type leaves original catalog")?;
            rotate_business_history(record, count)?;
            let state = u32::from(record[..4] != [1, 0, 0, 0]);
            record[0x6c..0x70].copy_from_slice(&state.to_le_bytes());
        }
        Ok(())
    }
}

/// Original00464100 updates eligible retail state and its byte countdown.
/// Opening/closing comparisons are signed and do not wrap across midnight.
pub fn update_hourly_businesses(businesses: &mut [[u8; 544]], hour: u32) {
    for business in businesses.iter_mut().take(600) {
        if business[0x21c] == 0 || business[..4] != [0; 4] {
            continue;
        }
        let state = u32::from_le_bytes(business[0x6c..0x70].try_into().unwrap());
        // The native compare-to-one branch tests the already-zero discriminator,
        // not state. Only state2 bypasses the schedule for eligible rows.
        if state != 2 {
            let opening = i32::from(business[0x68] as i8);
            let closing = i32::from(business[0x69] as i8);
            let closed = (hour as i32) < opening || (closing > opening && (hour as i32) >= closing);
            business[0x6c..0x70].copy_from_slice(&u32::from(closed).to_le_bytes());
        }
        if business[0x74] != 0 {
            business[0x74] -= 1;
        }
    }
}

/// Original00462b00. The caller supplies the building selected by the signed
/// business+8 index. Disabled accounting changes the owner byte only.
pub fn transfer_business_owner(
    business: &mut [u8; 544],
    building: &[u8; 64],
    new_owner: u8,
    accounting_enabled: bool,
    players: &mut [PlayerAccount],
) -> Result<(), String> {
    if accounting_enabled {
        let kind = u32::from_le_bytes(building[0x24..0x28].try_into().unwrap());
        let multiplier: i32 = if kind == 2 {
            100
        } else {
            match building[0x2c] {
                1 => 5,
                2 => 10,
                _ => 0,
            }
        };
        let volume = (i32::from(building[7]) - i32::from(building[5]) + 1)
            .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
            .wrapping_mul(i32::from(building[8]));
        let mut move_counter = |amount: i32| -> Result<(), String> {
            // Native tests signed-positive owner bytes, subtracts first, then
            // independently resolves and adds to the new owner (even if equal).
            for (owner, delta) in [(business[0xa], amount.wrapping_neg()), (new_owner, amount)] {
                if (owner as i8) > 0 {
                    let player = players
                        .get_mut(usize::from(owner) - 1)
                        .ok_or("business owner does not resolve to a player")?;
                    player.housing_counter = player.housing_counter.wrapping_add(delta);
                }
            }
            Ok(())
        };
        if multiplier != 0 {
            move_counter(volume.wrapping_mul(multiplier))?;
        }
        if business[..4] == 1u32.to_le_bytes() {
            let occupants = u16::from_le_bytes(business[0x10..0x12].try_into().unwrap());
            move_counter(i32::from(occupants) * 50)?;
        }
    }
    business[0xa] = new_owner;
    Ok(())
}

pub struct ResidentModelBinding {
    pub reference: u32,
    pub definition: [u8; 0x74],
}

impl ResidentModelBinding {
    /// Bind an opaque runtime handle to bounds rebuilt from the original mesh.
    pub fn from_mesh(reference: u32, mesh: &crate::mesh::CachedMesh<'_>) -> Result<Self, String> {
        Ok(Self {
            reference,
            definition: mesh.model_definition()?,
        })
    }
}

pub struct ResidentCreationState {
    pub auxiliary: crate::resident::AuxiliaryPool,
    pub initialization_rng: crate::resident::InitializationRng,
    pub identity_rng: u32,
    pub preference_rng: u32,
}

pub struct ResidentCreationInputs<'a> {
    pub personal: &'a crate::resident::InitializationInputs<'a>,
    pub preference_groups: &'a crate::resident::PreferenceGroups,
    pub models: &'a [ResidentModelBinding; 32],
    pub countdown_period: i32,
}

/// Original business-settings command0x18. Three transported dwords contain
/// six signed stock targets consumed by00461c40; prices use catalog order.
#[derive(Clone)]
pub struct BusinessTerms {
    pub stock_targets: [i16; 6],
    pub prices: [f32; 6],
    pub rent: f32,
}

/// Typed payload of the original0x18 command. The target is the source
/// business's stored+210 identity, which need not equal its current pool slot.
#[derive(Clone)]
pub struct PendingBusinessTerms {
    pub business_index: u32,
    pub terms: BusinessTerms,
}

/// Local command18 adapter over the shared original delayed-command queue.
#[derive(Default)]
pub struct LocalBusinessCommands {
    queue: crate::local_commands::Queue<PendingBusinessTerms>,
}
impl LocalBusinessCommands {
    /// Submit the current dirty-business batch through the local delayed path.
    /// The application callback receives the same business pool, including the
    /// immediate application that occurs when timestamp addition wraps.
    pub fn submit_pending(
        &mut self,
        businesses: &mut [[u8; 544]],
        player: i32,
        tick: u32,
        mut apply: impl FnMut(&PendingBusinessTerms, &mut [[u8; 544]]) -> Result<(), String>,
    ) -> Result<(), String> {
        submit_business_terms(businesses, player, |command, businesses| {
            self.submit(command, tick, true, |command| apply(command, businesses))
        })
    }

    pub fn submit(
        &mut self,
        command: PendingBusinessTerms,
        tick: u32,
        delayed: bool,
        apply: impl FnMut(&PendingBusinessTerms) -> Result<(), String>,
    ) -> Result<(), String> {
        self.queue.submit(command, tick, delayed, apply)
    }
    pub fn drain(
        &mut self,
        clocks: &mut [u32; 2],
        apply: impl FnMut(&PendingBusinessTerms) -> Result<(), String>,
    ) -> Result<(), String> {
        self.queue.drain(clocks, apply)
    }
}

/// Original00464060 submits dirty, active businesses belonging to this signed
/// zero-based player index. Clear dirty before submission; do not apply inline
/// here. Original local dispatch timestamps these commands25 ticks ahead.
pub fn submit_business_terms(
    businesses: &mut [[u8; 544]],
    player: i32,
    mut submit: impl FnMut(PendingBusinessTerms, &mut [[u8; 544]]) -> Result<(), String>,
) -> Result<(), String> {
    if businesses.len() != 600 {
        return Err("business submission requires original600-slot pool".into());
    }
    for slot in 0..600 {
        let record = &mut businesses[slot];
        if record[0x21c] == 0 || i32::from(record[10] as i8) - 1 != player || record[0x208] == 0 {
            continue;
        }
        let command = PendingBusinessTerms {
            business_index: u32::from_le_bytes(record[0x210..0x214].try_into().unwrap()),
            terms: BusinessTerms {
                stock_targets: std::array::from_fn(|i| {
                    i16::from_le_bytes(record[0x1e0 + i * 2..0x1e2 + i * 2].try_into().unwrap())
                }),
                prices: std::array::from_fn(|i| {
                    f32::from_le_bytes(record[0x1ec + i * 4..0x1f0 + i * 4].try_into().unwrap())
                }),
                rent: f32::from_le_bytes(record[0x204..0x208].try_into().unwrap()),
            },
        };
        record[0x208] = 0;
        submit(command, businesses)?;
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResidentCreationError {
    InvalidInputs(String),
    AuxiliaryExhausted { resident: usize },
    AccountsExhausted { resident: usize },
}

impl From<String> for ResidentCreationError {
    fn from(value: String) -> Self {
        Self::InvalidInputs(value)
    }
}

/// Current building/catalog inputs for one daily business update. Rebuild
/// these when building dimensions, ownership or catalog rules change.
pub struct BusinessDayParameters {
    pub capacity: i32,
    pub catalog_mask: u32,
    pub prices: Vec<f32>,
    pub utility_scalar: i32,
    pub utility_rates: [f32; 4],
    pub landlord: u8,
}

/// Saved BusinessInfo values; catalog order and commodity prices are shared
/// with shopping rather than reconstructed from a product bit mask.
pub struct BusinessDaySettings {
    pub capacity_rates: [f32; 45],
    pub utility_rates: [[f32; 4]; 45],
    pub catalog_masks: [u32; 45],
}

impl BusinessDaySettings {
    /// Script-final BusinessInfo fields consumed by the original daily pass.
    /// Rebuild after script changes, including EndOfDay callbacks.
    pub fn from_catalog(catalog: &crate::business_pool::BusinessCatalog) -> Self {
        let word = |kind: usize, offset: usize| {
            u32::from_le_bytes(catalog.rows()[kind][offset..offset + 4].try_into().unwrap())
        };
        Self {
            capacity_rates: std::array::from_fn(|kind| f32::from_bits(word(kind, 0x58))),
            utility_rates: std::array::from_fn(|kind| {
                std::array::from_fn(|i| f32::from_bits(word(kind, 0x60 + i * 4)))
            }),
            catalog_masks: std::array::from_fn(|kind| word(kind, 0x70)),
        }
    }
}

/// Current world inputs for463d40. Rebuild parameters immediately before the
/// daily pass, after scripts/expiry may have changed buildings or land owners.
pub struct BusinessDayInputs<'a> {
    pub buildings: &'a [[u8; 64]],
    pub building_categories: &'a [i16],
    pub category_owners: &'a [u8; 52],
    pub catalogs: &'a crate::shopping::Catalogs,
    pub unit_prices: &'a [f32; 32],
    pub settings: &'a BusinessDaySettings,
}

impl BusinessDayInputs<'_> {
    /// Resolve the current business geometry/catalog for00464240. Like the
    /// original, a negative quantity returns before touching those lookups.
    pub fn adjust_stock_target(
        &self,
        record: &mut [u8; 544],
        slot: usize,
        quantity: i16,
    ) -> Result<(), String> {
        if quantity < 0 {
            return Ok(());
        }
        let kind = u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize;
        let building_slot = i16::from_le_bytes(record[8..10].try_into().unwrap()) as usize;
        let building = self
            .buildings
            .get(building_slot)
            .ok_or("stock adjustment building leaves pool")?;
        let capacity = *self
            .settings
            .capacity_rates
            .get(kind)
            .ok_or("stock adjustment type leaves settings")?;
        let catalog = self
            .catalogs
            .commodities
            .get(kind)
            .ok_or("stock adjustment type leaves catalog")?;
        let fraction = *self
            .catalogs
            .fractions
            .get(kind)
            .and_then(|row| row.get(slot))
            .ok_or("stock adjustment fraction is unresolved")?;
        adjust_stock_target(
            record, building, capacity, catalog, fraction, slot, quantity,
        )
    }

    pub fn parameters(
        &self,
        records: &[[u8; 544]],
    ) -> Result<Vec<Option<BusinessDayParameters>>, String> {
        if records.len() != 600
            || self.buildings.len() != 600
            || self.building_categories.len() != 600
        {
            return Err("daily world inputs require original 600-slot pools".into());
        }
        records
            .iter()
            .map(|record| {
                if record[0x21c] == 0 {
                    return Ok(None);
                }
                let kind = u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize;
                let catalog = self
                    .catalogs
                    .commodities
                    .get(kind)
                    .ok_or("daily business type leaves catalog")?;
                if catalog.len() > 6 {
                    return Err("daily catalog exceeds original six slots".into());
                }
                let slot = i16::from_le_bytes(record[8..10].try_into().unwrap()) as usize;
                let building = self
                    .buildings
                    .get(slot)
                    .ok_or("daily business building leaves pool")?;
                let category = self.building_categories[slot] as usize;
                let landlord = *self
                    .category_owners
                    .get(category)
                    .ok_or("daily building has no category")?;
                let area = (i32::from(building[6]) - i32::from(building[4]) + 1)
                    .wrapping_mul(i32::from(building[7]) - i32::from(building[5]) + 1);
                let mut prices = Vec::new();
                for &id in catalog {
                    let price = *self
                        .unit_prices
                        .get(id as usize)
                        .ok_or("daily commodity leaves price table")?;
                    if record[..4] == [0; 4] {
                        prices.push(price);
                    }
                }
                Ok(Some(BusinessDayParameters {
                    capacity: stock_capacity(area, building[8], self.settings.capacity_rates[kind]),
                    catalog_mask: self.settings.catalog_masks[kind],
                    prices,
                    utility_scalar: area.wrapping_mul(i32::from(building[8])),
                    utility_rates: self.settings.utility_rates[kind],
                    landlord,
                }))
            })
            .collect()
    }
}

/// 00463d40: visit the original business table in source order, performing
/// income/replenishment, common rent, then utilities for each active record.
/// Returns every cash posting, including zero amounts, in execution order.
/// Player resets and aggregate owner payouts belong to the later caller.
pub fn update_daily_businesses(
    records: &mut [[u8; 0x220]],
    parameters: &[Option<BusinessDayParameters>],
    residents: &mut ResidentAccounts,
    players: &mut [PlayerAccount],
    utilities: &mut Utilities,
) -> Result<Vec<AccountPosting>, String> {
    if records.len() != 600 || parameters.len() != records.len() {
        return Err("daily business update requires the original 600-slot table".into());
    }
    let mut postings = Vec::new();
    for (record, parameters) in records.iter_mut().zip(parameters) {
        if record[0x21c] == 0 {
            continue;
        }
        let parameters = parameters
            .as_ref()
            .ok_or("active business lacks daily inputs")?;
        let owner = record[10];
        for id in [owner, parameters.landlord] {
            if id > 6 || usize::from(id) > players.len() {
                return Err("daily business owner does not resolve to a player".into());
            }
        }
        let account = if owner == 0 {
            None
        } else {
            players.get_mut(usize::from(owner) - 1)
        };
        match u32::from_le_bytes(record[..4].try_into().unwrap()) {
            0 => {
                let result = replenish_business_record(
                    record,
                    &parameters.prices,
                    parameters.capacity,
                    parameters.catalog_mask,
                    false,
                    account,
                )?;
                if owner != 0 {
                    postings.push(AccountPosting {
                        player: usize::from(owner) - 1,
                        amount: (-result.cost) as f32,
                    });
                }
            }
            1 => {
                let amount = residents.collect_housing_income(record, account)?;
                if owner != 0 {
                    postings.push(AccountPosting {
                        player: usize::from(owner) - 1,
                        amount,
                    });
                }
            }
            _ => {}
        }
        let float = |at| f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let mut business = BusinessAccount {
            cost_balance: float(0x30),
            balance: float(0x1c),
        };
        let mut rent_balance = float(0x28);
        let rent = u16::from_le_bytes(record[0x18..0x1a].try_into().unwrap());
        let expense_offsets = [0x44, 0x38, 0x40, 0x3c];
        let mut expenses = expense_offsets.map(float);
        postings.extend(business.settle_rent(
            &mut rent_balance,
            rent,
            owner,
            parameters.landlord,
            players,
        )?);
        let amounts = utilities.assess(parameters.utility_scalar, parameters.utility_rates, owner);
        let account = if owner == 0 {
            None
        } else {
            players.get_mut(usize::from(owner) - 1)
        };
        business.charge_utilities(&mut expenses, amounts, true, account)?;
        if owner != 0 {
            postings.extend(amounts.map(|amount| AccountPosting {
                player: usize::from(owner) - 1,
                amount: -amount,
            }));
        }
        record[0x28..0x2c].copy_from_slice(&rent_balance.to_le_bytes());
        record[0x1c..0x20].copy_from_slice(&business.balance.to_le_bytes());
        for (at, value) in expense_offsets.into_iter().zip(expenses) {
            record[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(postings)
}

impl ResidentAccounts {
    /// Original0045bd40 visits the signed count prefix, including inactive
    /// slots, and reloads the count after each00460470 call. The callback has
    /// access to the same accounts/pool so preceding changes remain visible.
    pub fn advance_population(
        &mut self,
        mut tick: impl FnMut(usize, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut index = 0_usize;
        while (index as i32) < self.active_count as i32 {
            if index >= self.nodes.len() {
                return Err("resident population visit leaves pool".into());
            }
            tick(index, self)?;
            index += 1;
        }
        Ok(())
    }

    /// Original00460470 with callbacks borrowing the authoritative pool.
    /// Housing callbacks can update occupancy chains without detaching the
    /// current resident. Re-read its record after each callback, as native does.
    pub fn tick_resident(
        &mut self,
        index: usize,
        auxiliary: &mut [u8; 284],
        centers: &[[f32; 2]; 52],
        mut callback: impl FnMut(
            crate::resident::TickEvent,
            usize,
            &mut Self,
            &mut [u8; 284],
        ) -> Result<bool, String>,
    ) -> Result<(), String> {
        use crate::resident::TickEvent;
        let resident = self.nodes.get(index).ok_or("resident tick leaves pool")?;
        if resident.as_bytes()[0x7e] & 4 != 0 {
            let done = callback(TickEvent::Movement, index, self, auxiliary)?;
            self.nodes[index].finish_movement_update(auxiliary, centers, done)?;
        }
        if self.nodes[index].advance_tick_countdown() {
            callback(TickEvent::RefreshIntent, index, self, auxiliary)?;
            self.nodes[index].finish_intent_refresh();
        }
        if self.nodes[index].take_tick_action() {
            callback(TickEvent::Action, index, self, auxiliary)?;
        }
        Ok(())
    }

    pub(crate) fn iteration_len(&self) -> Result<usize, String> {
        let count = (self.active_count as i32).max(0) as usize;
        if count > self.nodes.len() {
            return Err("resident iteration count leaves pool".into());
        }
        Ok(count)
    }

    ///0045bb80 through model binding. Optional event registration after
    /// binding is caller-owned. Native null-pointer faults become explicit
    /// errors after preserving the original preceding mutations.
    pub fn create_resident(
        &mut self,
        requested: [i32; 3],
        mut template: [u8; 0x11c],
        inputs: &ResidentCreationInputs<'_>,
        state: &mut ResidentCreationState,
        categories: &mut (impl crate::resident::PlacementCategories + ?Sized),
    ) -> Result<Option<usize>, ResidentCreationError> {
        if self.nodes.len() != 2000 || self.accounts.len() != 2000 || inputs.countdown_period <= 0 {
            return Err(ResidentCreationError::InvalidInputs(
                "creation requires complete pools and positive countdown period".into(),
            ));
        }
        let Some(index) = self.allocate_resident_slot()? else {
            return Ok(None);
        };
        self.active_count = self.active_count.wrapping_add(1);
        crate::resident::initialize_auxiliary_template(
            &mut template,
            self.nodes[index].pool_index(),
        );
        let auxiliary = state.auxiliary.allocate(&template);
        let account_index = self.allocate_account()?;
        let node = &mut self.nodes[index];
        node.set_account_index(account_index);
        let Some(auxiliary_index) = auxiliary else {
            node.reset_runtime_prefix(0);
            return Err(ResidentCreationError::AuxiliaryExhausted { resident: index });
        };
        // Nonzero slot+1 is the shared-core opaque auxiliary reference.
        let reference = (auxiliary_index + 1) as u32;
        let auxiliary = state.auxiliary.get_mut(auxiliary_index).unwrap();
        node.initialize_runtime(
            reference,
            auxiliary,
            &mut state.initialization_rng.constructor,
            inputs.countdown_period,
            categories,
        )?;
        let identity = inputs
            .preference_groups
            .resolve(
                requested,
                &mut state.identity_rng,
                &mut state.preference_rng,
            )?
            .map(|v| v as u32);
        if let Some(account_index) = account_index {
            node.initialize_personal(
                identity,
                &mut self.accounts[account_index],
                inputs.personal,
                &mut state.initialization_rng,
            )?;
        } else {
            node.initialize_personal_state(
                identity,
                inputs.personal,
                &mut state.initialization_rng,
            )?;
            return Err(ResidentCreationError::AccountsExhausted { resident: index });
        }
        let model = &inputs.models[node.model_variant() as usize];
        crate::resident::bind_auxiliary_model(auxiliary, model.reference, &model.definition);
        Ok(Some(index))
    }

    ///0045be80: fresh creation side effects precede a copy into the saved slot.
    /// The saved +148 word selects the destination independently of the newly
    /// allocated slot. Its active byte and runtime auxiliary/model survive.
    /// Optional creation event registration remains caller-owned.
    pub fn load_resident(
        &mut self,
        saved: [u8; 0x14a],
        template: [u8; 0x11c],
        inputs: &ResidentCreationInputs<'_>,
        state: &mut ResidentCreationState,
        categories: &mut [crate::housing::Category; 52],
        category_centers: &[[f32; 2]; 52],
    ) -> Result<usize, ResidentCreationError> {
        self.create_resident([-1; 3], template, inputs, state, categories)?;
        let index = usize::from(u16::from_le_bytes(saved[0x148..0x14a].try_into().unwrap()));
        let node = self
            .nodes
            .get_mut(index)
            .ok_or_else(|| "saved resident slot leaves pool".to_string())?;
        node.overwrite_saved_payload(saved);
        let reference = node.auxiliary_reference();
        let auxiliary = reference
            .checked_sub(1)
            .and_then(|index| state.auxiliary.get_mut(index as usize))
            .ok_or_else(|| "saved resident slot has no allocated auxiliary".to_string())?;
        node.restore_saved_movement(reference, auxiliary, category_centers)?;
        Ok(index)
    }

    /// Zero-filled process storage followed by the original slot/account resets.
    /// Restarting an existing city must use the resets to preserve untouched bytes.
    pub fn new_pool() -> Self {
        let mut result = Self {
            active_count: 0,
            nodes: (0..2000)
                .map(|_| crate::resident::Resident::from_pool_bytes([0; 0x14a], 0))
                .collect(),
            accounts: (0..2000)
                .map(|_| crate::resident::SpendingAccount::from_bytes([0; 0x90]))
                .collect(),
        };
        result.reset_resident_slots().unwrap();
        result
    }

    ///0045bac0 slot loop only: index words and active bytes, not auxiliary storage.
    pub fn reset_resident_slots(&mut self) -> Result<(), String> {
        if self.nodes.len() != 2000 {
            return Err("native resident pool requires2000 slots".into());
        }
        for (index, node) in self.nodes.iter_mut().enumerate() {
            node.reset_pool_slot(index as u16);
        }
        Ok(())
    }

    ///0045bb10 first-free scan; the higher-level constructor owns active count.
    pub fn allocate_resident_slot(&mut self) -> Result<Option<usize>, String> {
        if self.nodes.len() != 2000 {
            return Err("native resident pool requires2000 slots".into());
        }
        let Some(index) = self.nodes.iter().position(|node| !node.is_active()) else {
            return Ok(None);
        };
        self.nodes[index].mark_allocated();
        Ok(Some(index))
    }
    /// Original0045b2f0 clears all2000 account rows, preserving byte padding.
    pub fn reset_account_pool(&mut self) -> Result<(), String> {
        if self.accounts.len() != 2000 {
            return Err("native account pool requires2000 slots".into());
        }
        for account in &mut self.accounts {
            account.reset_slot(false);
        }
        Ok(())
    }

    /// Original0045b330 first-free scan. Exhaustion leaves every row unchanged.
    pub fn allocate_account(&mut self) -> Result<Option<usize>, String> {
        if self.accounts.len() != 2000 {
            return Err("native account pool requires2000 slots".into());
        }
        let Some(index) = self.accounts.iter().position(|a| !a.is_active()) else {
            return Ok(None);
        };
        self.accounts[index].reset_slot(true);
        Ok(Some(index))
    }

    fn housing_chain(&self, record: &[u8; 0x220]) -> Result<Vec<usize>, String> {
        let mut next = i32::from_le_bytes(record[0x20c..0x210].try_into().unwrap());
        let mut chain = Vec::new();
        let mut seen = vec![false; self.nodes.len()];
        while next != -1 {
            let index = usize::try_from(next).map_err(|_| "negative housing node index")?;
            let node = self
                .nodes
                .get(index)
                .ok_or("housing chain leaves resident pool")?;
            if seen[index] || usize::from(node.pool_index()) != index {
                return Err("cyclic housing chain or mismatched resident identity".into());
            }
            seen[index] = true;
            chain.push(index);
            next = node.next_in_housing();
        }
        Ok(chain)
    }

    /// Original 004617c0. Availability is the business-index order of 7f3da8.
    /// The host may issue the 801dec-gated notification after a successful
    /// residential append. This helper does not change the resident's address.
    pub fn append_housing_resident(
        &mut self,
        record: &mut [u8; 0x220],
        business_index: usize,
        resident_index: usize,
        available: &mut Vec<usize>,
    ) -> Result<bool, String> {
        let capacity = u16::from_le_bytes(record[0xe..0x10].try_into().unwrap());
        let occupied = u16::from_le_bytes(record[0x10..0x12].try_into().unwrap());
        let residential = u32::from_le_bytes(record[..4].try_into().unwrap()) == 1;
        if occupied >= capacity {
            if residential {
                remove_available_business(available, business_index);
            }
            return Ok(false);
        }
        if residential {
            let node = self
                .nodes
                .get(resident_index)
                .ok_or("resident outside pool")?;
            if usize::from(node.pool_index()) != resident_index {
                return Err("mismatched resident identity".into());
            }
            let chain = self.housing_chain(record)?;
            if chain.contains(&resident_index) {
                return Err("resident is already in this housing chain".into());
            }
            let id = i32::from(node.pool_index());
            if let Some(&last) = chain.last() {
                self.nodes[last].set_next_in_housing(id);
            } else {
                record[0x20c..0x210].copy_from_slice(&id.to_le_bytes());
            }
            self.nodes[resident_index].set_next_in_housing(-1);
            if occupied + 1 >= capacity {
                remove_available_business(available, business_index);
            }
        }
        record[0x10..0x12].copy_from_slice(&occupied.wrapping_add(1).to_le_bytes());
        Ok(true)
    }

    /// Original 004618c0. Resolve owner from signed business byte +a before
    /// calling; a null original player pointer is represented by None. Available
    /// businesses move to the list tail even if they already had vacant units.
    /// Host notifications and the caller's address/objective updates are separate.
    pub fn remove_housing_resident(
        &mut self,
        record: &mut [u8; 0x220],
        business_index: usize,
        resident_index: usize,
        available: &mut Vec<usize>,
        owner: Option<&mut PlayerAccount>,
    ) -> Result<bool, String> {
        if i32::from_le_bytes(record[0x20c..0x210].try_into().unwrap()) == -1 {
            return Ok(false);
        }
        if u32::from_le_bytes(record[..4].try_into().unwrap()) != 1 {
            return Ok(true);
        }
        let chain = self.housing_chain(record)?;
        let Some(position) = chain.iter().position(|&id| id == resident_index) else {
            return Ok(false);
        };
        let next = chain
            .get(position + 1)
            .map_or(-1, |&id| i32::from(self.nodes[id].pool_index()));
        if position == 0 {
            record[0x20c..0x210].copy_from_slice(&next.to_le_bytes());
        } else {
            self.nodes[chain[position - 1]].set_next_in_housing(next);
        }
        self.nodes[resident_index].set_next_in_housing(-1);
        let occupied = u16::from_le_bytes(record[0x10..0x12].try_into().unwrap()).wrapping_sub(1);
        record[0x10..0x12].copy_from_slice(&occupied.to_le_bytes());
        let capacity = u16::from_le_bytes(record[0xe..0x10].try_into().unwrap());
        if occupied < capacity {
            remove_available_business(available, business_index);
            available.push(business_index);
        }
        if record[0xa] != 0
            && let Some(owner) = owner
        {
            owner.housing_counter = owner.housing_counter.wrapping_sub(50);
        }
        Ok(true)
    }

    pub fn from_saved(saved: &crate::save::SavedResidents<'_>) -> Self {
        Self {
            active_count: saved.nodes.len() as u32,
            nodes: saved
                .nodes
                .iter()
                .map(|r| crate::resident::Resident::from_bytes(**r))
                .collect(),
            accounts: saved
                .accounts
                .chunks_exact(0x90)
                .map(|r| crate::resident::SpendingAccount::from_bytes(r.try_into().unwrap()))
                .collect(),
        }
    }

    /// Financial prefix of the hour-5 resident pass: 0045f540..0045f5bb.
    /// Credit Welfare, store the f32 balance, then redistribute spending in
    /// node allocation order. Later resident status/counter updates are separate.
    pub fn update_daily_finances(
        &mut self,
        welfare: &crate::resident::Welfare,
        preferences: &crate::resident::Preferences,
        commodities: &[crate::resident::Commodity; 32],
    ) -> Result<(), String> {
        let count = self.iteration_len()?;
        for node in &self.nodes[..count] {
            Self::credit_resident(node, &mut self.accounts, welfare, preferences, commodities)?;
        }
        Ok(())
    }

    /// Complete original0045bd80 resident pass, called at hour5 by the scheduler.
    /// Caller supplies the objective RNG's current state, not a guessed save seed.
    pub fn update_daily(
        &mut self,
        welfare: &crate::resident::Welfare,
        preferences: &crate::resident::Preferences,
        commodities: &[crate::resident::Commodity; 32],
        objectives: &crate::resident::DailyObjectives,
        objective_rng: &mut u32,
    ) -> Result<(), String> {
        let count = self.iteration_len()?;
        for node in &mut self.nodes[..count] {
            Self::credit_resident(node, &mut self.accounts, welfare, preferences, commodities)?;
            node.update_daily_status(objectives, objective_rng);
        }
        Ok(())
    }

    fn credit_resident(
        node: &crate::resident::Resident,
        accounts: &mut [crate::resident::SpendingAccount],
        welfare: &crate::resident::Welfare,
        preferences: &crate::resident::Preferences,
        commodities: &[crate::resident::Commodity; 32],
    ) -> Result<(), String> {
        let index =
            usize::try_from(node.account_index()).map_err(|_| "negative resident account index")?;
        let account = accounts
            .get_mut(index)
            .ok_or("resident account leaves pool")?;
        let amount = welfare
            .amount(node.age(), node.class())
            .ok_or("invalid resident age/class")?;
        let preferences = preferences
            .get(node.preference_index())
            .ok_or("invalid resident preferences")?;
        if !amount.is_finite() {
            return Err("non-finite Welfare amount".into());
        }
        account.set_balance((f64::from(amount) + f64::from(account.balance())) as f32);
        account.redistribute(preferences, commodities)?;
        Ok(())
    }

    /// Discriminator-one branch 00463d83..00463e11, before common rent/utilities.
    /// The business's linked owner is resolved by the caller.
    pub fn collect_housing_income(
        &mut self,
        record: &mut [u8; 0x220],
        owner: Option<&mut PlayerAccount>,
    ) -> Result<f32, String> {
        if u32::from_le_bytes(record[..4].try_into().unwrap()) != 1
            || i16::from_le_bytes(record[8..10].try_into().unwrap()) == -1
        {
            return Err("business does not select the housing income branch".into());
        }
        let mut node = i32::from_le_bytes(record[0x20c..0x210].try_into().unwrap());
        let mut seen = vec![false; self.nodes.len()];
        let mut chain = Vec::new();
        while node != -1 {
            let index = usize::try_from(node).map_err(|_| "negative resident node index")?;
            let resident = self
                .nodes
                .get(index)
                .ok_or("resident chain leaves node pool")?;
            let (ledger, next) = (resident.account_index(), resident.next_in_housing());
            if seen[index] {
                return Err("cyclic resident chain".into());
            }
            seen[index] = true;
            let ledger = usize::try_from(ledger).map_err(|_| "negative resident ledger index")?;
            if ledger >= self.accounts.len() {
                return Err("resident ledger index exceeds pool".into());
            }
            chain.push(ledger);
            node = next;
        }
        let rate = f32::from_le_bytes(record[0x1dc..0x1e0].try_into().unwrap());
        let mut total = 0f32;
        for ledger in chain {
            let account = &mut self.accounts[ledger];
            account.set_balance((f64::from(account.balance()) - f64::from(rate)) as f32);
            total = (f64::from(rate) + f64::from(total)) as f32;
        }
        for at in [0x50, 0x1c] {
            let before = f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
            record[at..at + 4]
                .copy_from_slice(&((f64::from(total) + f64::from(before)) as f32).to_le_bytes());
        }
        if let Some(owner) = owner {
            owner.post(total, 0x21)?;
        }
        record.copy_within(0x10..0x12, 0x12);
        Ok(total)
    }
}

// Original 004633c0 removes only the first matching list entry.
fn remove_available_business(available: &mut Vec<usize>, business_index: usize) {
    if let Some(position) = available.iter().position(|&id| id == business_index) {
        available.remove(position);
    }
}

/// Utility order throughout this type is telecom, gas, water, electrical.
/// Original aggregate indices for that order are 3, 2, 0, 1.
#[derive(Clone, Debug)]
pub struct Utilities {
    pub owners: [u8; 4],
    pub enabled: [bool; 4],
    pub totals: [f32; 4],
}

impl Utilities {
    /// 0048dfc0 pays each service in descending native index order, then
    /// clears its accumulated total. The global +86d518 factor is caller state.
    pub fn pay_owners(
        &mut self,
        players: &mut [PlayerAccount],
        factor: f32,
    ) -> Result<Vec<AccountPosting>, String> {
        for &owner in &self.owners {
            if owner > 6 || usize::from(owner) > players.len() {
                return Err("utility owner does not resolve to a native player".into());
            }
        }
        let mut postings = Vec::new();
        // Our service order is telecom/gas/water/electrical; original 3,2,1,0.
        for i in [0, 1, 3, 2] {
            let owner = self.owners[i];
            if owner != 0 {
                let count = self.owners.iter().filter(|&&id| id == owner).count();
                let amount =
                    (count as f64 * f64::from(self.totals[i]) * 0.25 * f64::from(factor)) as f32;
                let player = usize::from(owner) - 1;
                players[player].post(amount, [28, 27, 25, 26][i])?;
                postings.push(AccountPosting { player, amount });
            }
            self.totals[i] = 0.;
        }
        Ok(postings)
    }

    /// Original 0048dda0 process initialization: all four services enabled,
    /// accumulated totals zero. The original load reset also restores these
    /// fields; use OwnerIncomeSettings::reset_income to reset property income too.
    pub fn new(owners: [u8; 4]) -> Self {
        Self {
            owners,
            enabled: [true; 4],
            totals: [0.; 4],
        }
    }

    /// 00462070 rounds the integer footprint-height scalar to f32 before
    /// multiplying each saved rate. Ownership exempts the matching service.
    pub fn assess(&mut self, scalar: i32, rates: [f32; 4], owner: u8) -> [f32; 4] {
        let scalar = scalar as f32;
        let amounts = std::array::from_fn(|i| {
            if self.enabled[i] && owner != self.owners[i] {
                (f64::from(scalar) * f64::from(rates[i])) as f32
            } else {
                0.
            }
        });
        for (i, amount) in amounts.iter().enumerate() {
            if self.enabled[i] {
                self.totals[i] = (f64::from(self.totals[i]) + f64::from(*amount)) as f32;
            }
        }
        amounts
    }
}

/// Selected globals 0086d518 / 0086d514. Custom script values may be supplied
/// explicitly; the original hub baseline below does not execute Lua.
#[derive(Clone, Copy, Debug)]
pub struct OwnerIncomeSettings {
    pub multiplier: f32,
    pub daily_freight: f32,
}

impl OwnerIncomeSettings {
    /// 0048de60, reached by save loader 004bcd48 -> 004b9f20 after difficulty
    /// selection: clear utility totals, enable services, and seed freight.
    pub fn reset_income(self, utilities: &mut Utilities, properties: &mut PropertyIncome) {
        utilities.totals = [0.; 4];
        utilities.enabled = [true; 4];
        properties.totals = [self.daily_freight; 4];
    }

    /// The three accounting calls at 0047f569..0047f582, after EndOfDay Lua.
    /// This does not execute hourly callbacks, customer activity, or the clock.
    pub fn settle_day(
        self,
        records: &mut [[u8; 0x220]],
        parameters: &[Option<BusinessDayParameters>],
        residents: &mut ResidentAccounts,
        players: &mut [PlayerAccount],
        utilities: &mut Utilities,
        properties: &mut PropertyIncome,
    ) -> Result<(), String> {
        update_daily_businesses(records, parameters, residents, players, utilities)?;
        for player in players.iter_mut() {
            player.finish_daily_update();
        }
        self.pay_owners(utilities, properties, players).map(|_| ())
    }

    /// 0048e070: utilities, four-property freight, then re-enable services.
    /// The business update and player modifier reset happen earlier in the
    /// original daily callback and remain the caller's responsibility.
    pub fn pay_owners(
        self,
        utilities: &mut Utilities,
        properties: &mut PropertyIncome,
        players: &mut [PlayerAccount],
    ) -> Result<Vec<AccountPosting>, String> {
        for &owner in utilities.owners.iter().chain(&properties.owners) {
            if owner > 6 || usize::from(owner) > players.len() {
                return Err("income owner does not resolve to a native player".into());
            }
        }
        let mut postings = utilities.pay_owners(players, self.multiplier)?;
        postings.extend(properties.pay_owners(players, self.multiplier, self.daily_freight)?);
        utilities.enabled = [true; 4];
        Ok(postings)
    }

    /// Unmodified supplied v1.4 DEFAULT/hub.lua lines 31..36. All three
    /// difficulty branches currently select the same values; commented
    /// alternative freight values are not active assignments.
    pub fn original_hub(difficulty: u32) -> Result<Self, String> {
        if difficulty > 2 {
            return Err("unsupported saved difficulty for original hub settings".into());
        }
        Ok(Self {
            multiplier: 1.,
            daily_freight: 800.,
        })
    }
}

/// 0048df00's separate four-property income, in original category-table order
/// [4,17,43,44]. Totals reside at manager +0x10..+0x1c.
#[derive(Clone, Debug)]
pub struct PropertyIncome {
    pub owners: [u8; 4],
    pub totals: [f32; 4],
}

impl PropertyIncome {
    /// Original0048ded0 adds arrival income even when the property is unowned.
    /// The resident's special home categories map to slots0..3 and amount5.
    pub fn record_arrival(&mut self, index: usize, amount: f32) -> Result<(), String> {
        let total = self
            .totals
            .get_mut(index)
            .ok_or("arrival property index leaves table")?;
        *total = (f64::from(amount) + f64::from(*total)) as f32;
        Ok(())
    }

    /// Payout precedes resetting each accumulator to global 0086d514.
    /// The factor is global 0086d518, shared with utility payouts.
    pub fn pay_owners(
        &mut self,
        players: &mut [PlayerAccount],
        factor: f32,
        baseline: f32,
    ) -> Result<Vec<AccountPosting>, String> {
        for &owner in &self.owners {
            if owner > 6 || usize::from(owner) > players.len() {
                return Err("property owner does not resolve to a native player".into());
            }
        }
        let mut postings = Vec::new();
        for i in (0..4).rev() {
            let owner = self.owners[i];
            if owner != 0 {
                let count = self.owners.iter().filter(|&&id| id == owner).count();
                let amount =
                    (count as f64 * f64::from(self.totals[i]) * 0.25 * f64::from(factor)) as f32;
                let player = usize::from(owner) - 1;
                players[player].post(amount, 29 + i)?;
                postings.push(AccountPosting { player, amount });
            }
            self.totals[i] = baseline;
        }
        Ok(postings)
    }
}

impl BusinessAccount {
    /// Reached rent branch 00463e1c. `rent_balance` is business +0x28,
    /// distinct from replenishment cost_balance (+0x30). Owners are raw
    /// one-based IDs; zero means unowned. The linked building resolves landlord.
    pub fn settle_rent(
        &mut self,
        rent_balance: &mut f32,
        rent: u16,
        tenant: u8,
        landlord: u8,
        players: &mut [PlayerAccount],
    ) -> Result<Vec<AccountPosting>, String> {
        for owner in [tenant, landlord] {
            if owner > 6 || usize::from(owner) > players.len() {
                return Err("rent owner does not resolve to a native player".into());
            }
        }
        let mut postings = Vec::new();
        if tenant == landlord {
            return Ok(postings);
        }
        let debit = -f32::from(rent);
        *rent_balance = (f64::from(*rent_balance) + f64::from(debit)) as f32;
        self.balance = (f64::from(self.balance) + f64::from(debit)) as f32;
        if tenant != 0 {
            let player = usize::from(tenant) - 1;
            players[player].post(debit, 0x0e)?;
            postings.push(AccountPosting {
                player,
                amount: debit,
            });
        }
        if landlord != 0 {
            let player = usize::from(landlord) - 1;
            let amount = f32::from(rent);
            players[player].post(amount, 0x22)?;
            postings.push(AccountPosting { player, amount });
        }
        Ok(postings)
    }

    /// Apply 00462070's four rounded amounts in original store/cash order.
    /// Expense balances correspond to business +44,+38,+40,+3c.
    pub fn charge_utilities(
        &mut self,
        expenses: &mut [f32; 4],
        amounts: [f32; 4],
        charge: bool,
        mut owner: Option<&mut PlayerAccount>,
    ) -> Result<(), String> {
        for (i, amount) in amounts.into_iter().enumerate() {
            let debit = -amount;
            expenses[i] = (f64::from(expenses[i]) + f64::from(debit)) as f32;
            if charge {
                self.balance = (f64::from(self.balance) + f64::from(debit)) as f32;
                if let Some(account) = owner.as_deref_mut() {
                    account.post(debit, [7, 6, 4, 5][i])?;
                }
            }
        }
        Ok(())
    }

    pub fn from_saved(business: &crate::save::SavedBusiness<'_>) -> Self {
        let float = |at| f32::from_le_bytes(business.bytes[at..at + 4].try_into().unwrap());
        Self {
            cost_balance: float(0x30),
            balance: float(0x1c),
        }
    }

    /// Apply 00461c40 stock and reached accounting operations. Returned
    /// turnover values belong in the business's selected history period.
    /// Catalog mask, history storage and daily scheduling remain caller work.
    pub fn replenish(
        &mut self,
        lines: &mut [RestockLine],
        capacity: i32,
        free: bool,
        mut owner: Option<&mut PlayerAccount>,
    ) -> Result<Replenishment, String> {
        if lines.len() > 6 {
            return Err("business exceeds six native commodity slots".into());
        }
        let modifier = owner.as_ref().map_or(0, |p| p.replenishment_modifier);
        let result = replenish_stock(lines, capacity, modifier);
        if !free {
            if let Some(account) = owner.as_deref_mut() {
                for (line, slot) in lines.iter().zip(&result.slots) {
                    let amount = (-f64::from(slot.turnover) * f64::from(line.unit_cost)) as f32;
                    account.record(amount, 0x13)?;
                }
            }
            // FST rounds the charge but leaves the extended value on x87
            // for the business cost addition (00461f39..00461f4b).
            let charge = (-result.cost) as f32;
            self.cost_balance = (f64::from(self.cost_balance) - result.cost) as f32;
            self.balance = (f64::from(self.balance) + f64::from(charge)) as f32;
            if let Some(account) = owner {
                account.post(charge, 0x11)?;
            }
        }
        for (line, slot) in lines.iter_mut().zip(&result.slots) {
            line.stock = slot.stock;
        }
        Ok(result)
    }
}

/// Apply original 004625c0 for a resolved catalog slot. Quantity is a signed
/// dword; stock/history word arithmetic wraps without an extra quantity clamp.
/// Buyer demand selection belongs to the caller. `tick` is global 00822d40.
/// Catalog identity/order must come from script initialization, not guessed.
/// Returns the rounded proprietor posting, or None when the shop has no owner.
pub fn purchase_business_record(
    record: &mut [u8; 0x220],
    slot: usize,
    commodity_id: u32,
    unit_cost: f32,
    quantity: i32,
    tick: u32,
    owner: Option<&mut PlayerAccount>,
) -> Result<Option<f32>, String> {
    let history = usize::from(record[0x209]);
    if slot >= 6 || commodity_id >= 32 || history >= 4 {
        return Err("unsupported retail catalog slot, commodity or history".into());
    }
    let at = slot * 0x34;
    let stock = u16::from_le_bytes(record[at + 0x80..at + 0x82].try_into().unwrap());
    let float = |offset| f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
    let price = float(0x1c4 + slot * 4);
    let q = crate::x87::Sum::from_i32(quantity);
    let mut proceeds = q;
    proceeds.mul_f32(price)?;
    let mut profit = crate::x87::Sum::from_f32(price)?;
    profit.add(crate::x87::Sum::from_f32(unit_cost)?.negated());
    profit.mul(q);
    profit.add_f32(float(at + 0x94))?;
    let add_proceeds = |old| -> Result<f32, String> {
        let mut sum = proceeds;
        sum.add_f32(old)?;
        Ok(sum.to_f32())
    };
    // FST at 004626b8 rounds the argument without discarding the x87 value.
    let updates = [
        (at + 0x94, profit.to_f32()),
        (at + 0xa4, add_proceeds(float(at + 0xa4))?),
        (0x24, add_proceeds(float(0x24))?),
        (
            0x1c,
            (f64::from(proceeds.to_f32()) + f64::from(float(0x1c))) as f32,
        ),
    ];
    for (offset, value) in updates {
        record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let remaining = stock.wrapping_sub(quantity as u16);
    record[at + 0x80..at + 0x82].copy_from_slice(&remaining.to_le_bytes());
    let total = i32::from_le_bytes(record[0x218..0x21c].try_into().unwrap());
    record[0x218..0x21c].copy_from_slice(&total.wrapping_sub(quantity).to_le_bytes());
    if remaining == 0 {
        let mask = u32::from_le_bytes(record[0x70..0x74].try_into().unwrap());
        record[0x70..0x74].copy_from_slice(&(mask & !(1u32 << commodity_id)).to_le_bytes());
    }
    let history_at = at + 0x82 + history * 2;
    let sold = i16::from_le_bytes(record[history_at..history_at + 2].try_into().unwrap());
    record[history_at..history_at + 2]
        .copy_from_slice(&sold.wrapping_add(quantity as i16).to_le_bytes());
    let posted = owner.is_some();
    if let Some(owner) = owner {
        owner.post(proceeds.to_f32(), 0x23)?;
        owner.retail_counters[0] = owner.retail_counters[0].wrapping_add(quantity.wrapping_mul(20));
        owner.retail_counters[1] = owner.retail_counters[1].wrapping_add(quantity);
    }
    let previous = u32::from_le_bytes(record[0x214..0x218].try_into().unwrap());
    let elapsed = tick.wrapping_sub(previous) as i32;
    if elapsed >= 100 || elapsed == 0 {
        record[0x214..0x218].copy_from_slice(&tick.to_le_bytes());
    }
    Ok(posted.then(|| proceeds.to_f32()))
}

/// Apply 00461c40 to a loaded business record. Prices retain runtime catalog
/// order; mask comes from BusinessInfo+70, independently of the current stock.
/// The linked owner and footprint-derived capacity are resolved by the caller.
pub fn replenish_business_record(
    record: &mut [u8; 0x220],
    prices: &[f32],
    capacity: i32,
    catalog_mask: u32,
    free: bool,
    owner: Option<&mut PlayerAccount>,
) -> Result<Replenishment, String> {
    let history = usize::from(record[0x209]);
    if history >= 4 || prices.len() > 6 {
        return Err("unsupported business history index or catalog count".into());
    }
    let float = |at| f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    let mut account = BusinessAccount {
        cost_balance: float(0x30),
        balance: float(0x1c),
    };
    let mut lines: Vec<_> = prices
        .iter()
        .enumerate()
        .map(|(i, &unit_cost)| {
            let stock_at = 0x80 + i * 0x34;
            let target_at = 0x1b8 + i * 2;
            RestockLine {
                stock: u16::from_le_bytes(record[stock_at..stock_at + 2].try_into().unwrap()),
                target: i16::from_le_bytes(record[target_at..target_at + 2].try_into().unwrap()),
                unit_cost,
            }
        })
        .collect();
    let result = account.replenish(&mut lines, capacity, free, owner)?;
    for (i, slot) in result.slots.iter().enumerate() {
        let stock_at = 0x80 + i * 0x34;
        let history_at = 0x8a + history * 2 + i * 0x34;
        record[history_at..history_at + 2].copy_from_slice(&slot.turnover.to_le_bytes());
        record[stock_at..stock_at + 2].copy_from_slice(&slot.stock.to_le_bytes());
    }
    record[0x70..0x74].copy_from_slice(&catalog_mask.to_le_bytes());
    if !free {
        record[0x30..0x34].copy_from_slice(&account.cost_balance.to_le_bytes());
        record[0x1c..0x20].copy_from_slice(&account.balance.to_le_bytes());
    }
    record[0x218..0x21c].copy_from_slice(&result.total_stock.to_le_bytes());
    Ok(result)
}

/// Original 004722f0 quote calculation, not a purchase or money transfer.
/// Preserve product order and the original per-product f32 accumulation store.
pub fn restocking_cost(lines: &[RestockLine]) -> f32 {
    let mut total = 0f32;
    for line in lines {
        let deficit = i32::from(line.target) - i32::from(line.stock);
        if deficit > 0 {
            total = (f64::from(total) + f64::from(deficit) * f64::from(line.unit_cost)) as f32;
        }
    }
    total
}

#[derive(Debug)]
pub struct ReplenishedSlot {
    pub stock: u16,
    pub turnover: u16,
    pub increment: i32,
}

#[derive(Debug)]
pub struct Replenishment {
    pub slots: Vec<ReplenishedSlot>,
    /// Kept across the loop in x87, unlike the per-slot f32 quote accumulator.
    pub cost: f64,
    pub total_stock: u32,
}

/// Stock portion of 00461c40. Capacity is the already-converted integer from
/// footprint * height * BusinessInfo.stockcapacity. Modifier is the original
/// linked object's +0x148: 1 halves cost, 2 halves individual increments.
/// Accounting, turnover history and scheduling are applied by the caller.
pub fn replenish_stock(lines: &[RestockLine], capacity: i32, modifier: u32) -> Replenishment {
    let mut remaining = capacity;
    let mut deficit = 0i32;
    let mut slots = Vec::with_capacity(lines.len());
    for line in lines {
        let turnover = line.stock / 2;
        let stock = line.stock.wrapping_sub(turnover);
        remaining = remaining.wrapping_sub(i32::from(stock));
        if i32::from(stock) < i32::from(line.target) {
            deficit = deficit.wrapping_add(i32::from(line.target) - i32::from(stock));
        }
        slots.push(ReplenishedSlot {
            stock,
            turnover,
            increment: 0,
        });
    }
    let factor = if deficit > 0 {
        ((remaining / deficit) as f32).min(1.)
    } else {
        1.
    };
    let mut cost = 0f64;
    for (line, slot) in lines.iter().zip(&mut slots) {
        if i32::from(slot.stock) < i32::from(line.target) {
            let difference = i32::from(line.target) - i32::from(slot.stock);
            let mut increment = ftol_low_word(f64::from(difference) * f64::from(factor));
            if modifier == 2 {
                increment /= 2;
            }
            slot.stock = slot.stock.wrapping_add(increment as u16);
            slot.increment = increment;
            cost += f64::from(line.unit_cost) * f64::from(increment);
        }
    }
    if modifier == 1 {
        cost *= 0.5;
    }
    let total_stock = slots
        .iter()
        .fold(0u32, |sum, s| sum.wrapping_add(u32::from(s.stock)));
    Replenishment {
        slots,
        cost,
        total_stock,
    }
}

/// Original 00461c67..00461c95 integer seed and _ftol conversion.
pub fn stock_capacity(area: i32, height: u8, capacity_per_unit: f32) -> i32 {
    ftol_low_word(f64::from(area.wrapping_mul(i32::from(height))) * f64::from(capacity_per_unit))
}

/// Original00464240 adjusts the pending stock word, without changing live
/// stock or live targets. Catalog traversal stops at its first-1 identity or
/// six entries; the selected fraction is resolved separately by the caller.
/// Quantity is a signed word: negative requests return before any lookup.
pub fn adjust_stock_target(
    record: &mut [u8; 544],
    building: &[u8; 64],
    capacity_per_unit: f32,
    catalog: &[u32],
    selected_fraction: f32,
    slot: usize,
    quantity: i16,
) -> Result<(), String> {
    if quantity < 0 {
        return Ok(());
    }
    if slot >= 6 {
        return Err("stock adjustment slot exceeds six words".into());
    }
    let area = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1);
    let capacity = stock_capacity(area, building[8], capacity_per_unit);
    let mut remaining = capacity;
    for (index, &commodity) in catalog.iter().take(6).enumerate() {
        if commodity == u32::MAX {
            break;
        }
        let at = 0x1e0 + index * 2;
        let old = i16::from_le_bytes(record[at..at + 2].try_into().unwrap());
        remaining = remaining.wrapping_sub(i32::from(old));
    }
    let at = 0x1e0 + slot * 2;
    let old = i16::from_le_bytes(record[at..at + 2].try_into().unwrap());
    // The original retains only EAX from the first_ftol before multiplying
    // the signed capacity by the selected f32 fraction and subtracting old.
    let selected =
        ftol_low_word(f64::from(capacity) * f64::from(selected_fraction) - f64::from(old));
    let delta = selected
        .min(remaining)
        .min(i32::from(quantity) - i32::from(old));
    record[at..at + 2].copy_from_slice(&old.wrapping_add(delta as i16).to_le_bytes());
    if delta != 0 {
        record[0x208] = 1;
    }
    Ok(())
}

/// The original _ftol returns an i64; these callers use only EAX. Invalid
/// FISTP conversions produce the integer-indefinite i64, whose low word is 0.
pub(crate) fn ftol_low_word(value: f64) -> i32 {
    if !value.is_finite()
        || !(-9223372036854775808.0..9223372036854775808.0).contains(&value.trunc())
    {
        0
    } else {
        (value.trunc() as i64) as i32
    }
}

/// 00462260 rotates one original 0x220-byte business record, preserving fields
/// outside the reached operation. Catalog identity/count is runtime script state.
pub fn rotate_business_history(record: &mut [u8; 0x220], catalog_count: u8) -> Result<(), String> {
    let old_index = usize::from(record[0x209]);
    if old_index >= 4 || catalog_count > 6 {
        return Err("unsupported business history index or catalog count".into());
    }
    record.copy_within(0x50..0x54, 0x14);
    let mut total = crate::x87::Sum::default();
    for at in (0x24..0x58).step_by(4) {
        total.add_f32(f32::from_le_bytes(record[at..at + 4].try_into().unwrap()))?;
    }
    let at = 0x58 + old_index * 4;
    record[at..at + 4].copy_from_slice(&total.to_f32().to_le_bytes());
    record[0x24..0x58].fill(0);
    let new_index = (old_index + 1) % 4;
    record[0x209] = new_index as u8;
    for slot in 0..usize::from(catalog_count) {
        let at = 0x94 + slot * 0x34;
        record.copy_within(at..at + 4, at + 4);
        record.copy_within(at + 0x10..at + 0x14, at + 0x14);
        record[at..at + 4].fill(0);
        record[at + 0x10..at + 0x14].fill(0);
        let cell = 0x82 + 2 * (new_index + 0x1a * slot);
        record[cell..cell + 2].fill(0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original home-arrival income fixture"]
    fn matches_native_home_arrival_income() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_HOME_ARRIVAL")).unwrap();
        assert_eq!(&data[..8], b"MTHINC01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let index = word(at) as usize;
            let amount = f32::from_bits(word(at + 4));
            at += 8;
            let before = &data[at..at + 36];
            at += 36;
            let expected = &data[at..at + 36];
            at += 36;
            let mut income = PropertyIncome {
                owners: [0; 4],
                totals: std::array::from_fn(|i| {
                    f32::from_le_bytes(before[16 + i * 4..20 + i * 4].try_into().unwrap())
                }),
            };
            income.record_arrival(index, amount).unwrap();
            let mut actual = before.to_vec();
            for (i, total) in income.totals.iter().enumerate() {
                actual[16 + i * 4..20 + i * 4].copy_from_slice(&total.to_le_bytes());
            }
            assert_eq!(actual, expected, "case{case}");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original Lua money binding observations"]
    fn matches_native_script_money() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_MONEY")).unwrap();
        assert_eq!(&bytes[..8], b"MTAICSH1");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let mut at = 12;
        for case in 0..count {
            let operation = match u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) {
                0 => super::ScriptMoneyOperation::Set,
                1 => super::ScriptMoneyOperation::Increase,
                2 => super::ScriptMoneyOperation::Decrease,
                _ => panic!("unknown money operation"),
            };
            let argument = f64::from_le_bytes(bytes[at + 4..at + 12].try_into().unwrap());
            at += 12;
            let mut player = bytes[at..at + 0x1c0].to_vec();
            let cash = f32::from_le_bytes(player[4..8].try_into().unwrap());
            player[4..8]
                .copy_from_slice(&super::script_money(cash, argument, operation).to_le_bytes());
            at += 0x1c0;
            assert_eq!(player, &bytes[at..at + 0x1c0], "money {case}");
            at += 0x1c0;
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original local business-command transport observations"]
    fn matches_native_local_business_commands() {
        check_local_business_commands("OPENMT2001_NATIVE_LOCAL_BUSINESS_COMMAND", false);
    }

    #[test]
    #[ignore = "requires original pending batch through local transport observations"]
    fn matches_native_local_business_batch() {
        check_local_business_commands("OPENMT2001_NATIVE_LOCAL_BUSINESS_BATCH", true);
    }

    fn check_local_business_commands(variable: &str, batch: bool) {
        let bytes = std::fs::read(crate::fixtures::path(variable)).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], if batch { b"MTLOCB02" } else { b"MTLOCAL1" });
        let mut at = 12;
        for _ in 0..word(8) {
            let config: [u32; 4] = std::array::from_fn(|i| word(at + i * 4));
            at += 16;
            let mut businesses: Vec<[u8; 544]> = Vec::new();
            let mut expected_businesses = &bytes[0..0];
            let command = if batch {
                businesses = bytes[at..at + 600 * 544]
                    .chunks_exact(544)
                    .map(|r| r.try_into().unwrap())
                    .collect();
                at += 600 * 544;
                expected_businesses = &bytes[at..at + 600 * 544];
                at += 600 * 544;
                None
            } else {
                Some(super::PendingBusinessTerms {
                    business_index: word(at),
                    terms: super::BusinessTerms {
                        stock_targets: std::array::from_fn(|i| {
                            i16::from_le_bytes(
                                bytes[at + 4 + i * 2..at + 6 + i * 2].try_into().unwrap(),
                            )
                        }),
                        prices: std::array::from_fn(|i| f32::from_bits(word(at + 16 + i * 4))),
                        rent: f32::from_bits(word(at + 40)),
                    },
                })
            };
            if !batch {
                at += 44;
            }
            let expected = &bytes[at..at + config[3] as usize * 52];
            at += expected.len();
            let mut actual = Vec::new();
            let mut queue = super::LocalBusinessCommands::default();
            let mut capture = |tick: u32, command: &super::PendingBusinessTerms| {
                actual.extend(tick.to_le_bytes());
                actual.extend(command.business_index.to_le_bytes());
                actual.extend(command.business_index.to_le_bytes());
                for value in command.terms.stock_targets {
                    actual.extend(value.to_le_bytes());
                }
                for value in command.terms.prices {
                    actual.extend(value.to_le_bytes());
                }
                actual.extend(command.terms.rent.to_le_bytes());
                Ok(())
            };
            if let Some(command) = command {
                queue
                    .submit(command, config[0], config[1] != 0, |command| {
                        capture(config[0], command)
                    })
                    .unwrap();
            } else {
                queue
                    .submit_pending(&mut businesses, 0, config[0], |command, _| {
                        capture(config[0], command)
                    })
                    .unwrap();
            }
            for step in 0..config[2] {
                let tick = config[0].wrapping_add(step);
                let result = queue.drain(&mut [tick; 2], |command| capture(tick, command));
                if config[1] & 2 != 0 {
                    assert!(result.unwrap_err().contains("exceeded4096"));
                } else {
                    result.unwrap();
                }
            }
            if batch {
                assert_eq!(businesses.as_flattened(), expected_businesses);
            }
            assert_eq!(
                actual, expected,
                "initial tick{:08x}, delayed{}",
                config[0], config[1]
            );
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original pending-business submission fixture"]
    fn matches_native_business_submission() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_SUBMISSION")).unwrap();
        assert_eq!(&bytes[..8], b"MTBSUB01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let player = word(at) as i32;
            let mutate = word(at + 4) != 0;
            at += 8;
            let mut businesses: Vec<[u8; 544]> = bytes[at..at + 600 * 544]
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 600 * 544;
            let identities: std::collections::BTreeMap<u32, usize> = businesses
                .iter()
                .enumerate()
                .map(|(slot, r)| {
                    (
                        u32::from_le_bytes(r[0x210..0x214].try_into().unwrap()),
                        slot,
                    )
                })
                .collect();
            let count = word(at) as usize;
            at += 4;
            let expected_commands = &bytes[at..at + count * 48];
            at += count * 48;
            let mut commands = Vec::new();
            super::submit_business_terms(&mut businesses, player, |command, businesses| {
                let source = identities[&command.business_index];
                assert_eq!(
                    businesses[source][0x208], 0,
                    "clear precedes submission case{case}"
                );
                commands.extend_from_slice(&(source as u32).to_le_bytes());
                commands.extend_from_slice(&command.business_index.to_le_bytes());
                for target in command.terms.stock_targets {
                    commands.extend_from_slice(&target.to_le_bytes());
                }
                for price in command.terms.prices {
                    commands.extend_from_slice(&price.to_le_bytes());
                }
                commands.extend_from_slice(&command.terms.rent.to_le_bytes());
                if mutate && commands.len() == 48 {
                    businesses[0][0x208] = 3;
                    businesses[599][0x208] = 2;
                    businesses[599][0x1ec..0x1f0].copy_from_slice(&91.125_f32.to_le_bytes());
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(
                commands, expected_commands,
                "command order/payload case{case}"
            );
            for record in businesses {
                assert_eq!(
                    &record[..],
                    &bytes[at..at + 544],
                    "business submission state case{case}"
                );
                at += 544;
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original stock-adjustment instructions fixture"]
    fn matches_native_stock_adjustment() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_STOCK_ADJUSTMENT")).unwrap();
        assert_eq!(&bytes[..8], b"MTSTADJ1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let slot = word(at) as usize;
            let quantity = word(at + 4) as i16;
            let building: &[u8; 64] = bytes[at + 8..at + 72].try_into().unwrap();
            let capacity = f32::from_bits(word(at + 72));
            let count = (word(at + 76) as usize).min(6);
            let catalog: [u32; 6] = std::array::from_fn(|i| word(at + 80 + i * 8));
            let fractions: [f32; 6] =
                std::array::from_fn(|i| f32::from_bits(word(at + 84 + i * 8)));
            at += 128;
            let mut record: [u8; 544] = bytes[at..at + 544].try_into().unwrap();
            let mut bound_record = record;
            at += 544;
            super::adjust_stock_target(
                &mut record,
                building,
                capacity,
                &catalog[..count],
                fractions.get(slot).copied().unwrap_or(0.),
                slot,
                quantity,
            )
            .unwrap();
            assert_eq!(
                &record[..],
                &bytes[at..at + 544],
                "stock adjustment case{case}"
            );
            let kind = u32::from_le_bytes(bound_record[4..8].try_into().unwrap()) as usize;
            let building_slot =
                i16::from_le_bytes(bound_record[8..10].try_into().unwrap()) as usize;
            let mut buildings = vec![[0; 64]; building_slot + 1];
            buildings[building_slot] = *building;
            let mut catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| vec![]),
                fractions: std::array::from_fn(|_| vec![]),
            };
            catalogs.commodities[kind] = catalog[..count].to_vec();
            catalogs.fractions[kind] = fractions.to_vec();
            let mut settings = super::BusinessDaySettings {
                capacity_rates: [0.; 45],
                utility_rates: [[0.; 4]; 45],
                catalog_masks: [0; 45],
            };
            settings.capacity_rates[kind] = capacity;
            super::BusinessDayInputs {
                buildings: &buildings,
                building_categories: &[],
                category_owners: &[0; 52],
                catalogs: &catalogs,
                unit_prices: &[0.; 32],
                settings: &settings,
            }
            .adjust_stock_target(&mut bound_record, slot, quantity)
            .unwrap();
            assert_eq!(
                bound_record, record,
                "stock adjustment world binding case{case}"
            );
            at += 544;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original business ownership transfer observations"]
    fn matches_native_business_owner_transfer() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_OWNER")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MT462B00");
        assert_eq!(word(8), 16);
        let mut at = 12;
        for _ in 0..word(8) {
            let length = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&bytes[at..at + length]).unwrap();
            at += length;
            let enabled = word(at) != 0;
            let new_owner = word(at + 4) as u8;
            at += 8;
            let mut phases = Vec::new();
            for _ in 0..2 {
                let mut records = Vec::new();
                for length in [544, 916] {
                    assert_eq!(word(at), length);
                    at += 4;
                    records.push(bytes[at..at + length as usize].to_vec());
                    at += length as usize;
                }
                assert_eq!(word(at), 4);
                at += 4;
                for id in [-1, 0, 1, 2] {
                    assert_eq!(word(at) as i32, id);
                    assert_eq!(word(at + 4), 444);
                    at += 8;
                    records.push(bytes[at..at + 444].to_vec());
                    at += 444;
                }
                phases.push(records);
            }
            let mut actual = phases.remove(0);
            let expected = phases.remove(0);
            let mut players: Vec<_> = actual[4..6]
                .iter()
                .map(|raw| {
                    PlayerAccount::from_saved(&crate::save::SavedPlayer {
                        metadata: &[0; 48],
                        bytes: raw[4..444].try_into().unwrap(),
                    })
                    .unwrap()
                })
                .collect();
            let mut business = actual[0].as_slice().try_into().unwrap();
            transfer_business_owner(
                &mut business,
                actual[1][..64].try_into().unwrap(),
                new_owner,
                enabled,
                &mut players,
            )
            .unwrap();
            actual[0].copy_from_slice(&business);
            for (raw, player) in actual[4..6].iter_mut().zip(players) {
                raw[0x1b0..0x1b4].copy_from_slice(&player.housing_counter.to_le_bytes());
            }
            assert_eq!(actual, expected, "{name} complete records");
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original hourly business table observations"]
    fn matches_native_hourly_business_state() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_HOUR")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTHOUR02");
        let mut at = 12;
        assert_eq!(word(8), 62);
        let mut loaded: Option<(super::Ledgers, crate::clock::Clock)> = None;
        for case in 0..word(8) {
            let hour = word(at);
            let name_len = word(at + 4) as usize;
            at += 8;
            let name = std::str::from_utf8(&bytes[at..at + name_len]).unwrap();
            at += name_len;
            let length = word(at) as usize;
            at += 4;
            assert_eq!(length, 600 * 544);
            let mut businesses: Vec<[u8; 544]> = bytes[at..at + length]
                .chunks_exact(544)
                .map(|record| record.try_into().unwrap())
                .collect();
            at += length;
            assert_eq!(word(at) as usize, length);
            at += 4;
            if case < 14 {
                super::update_hourly_businesses(&mut businesses, hour);
            } else {
                if (case - 14) % 24 == 0 {
                    let variable = if case == 14 {
                        "OPENMT2001_SAVE_ONE"
                    } else {
                        "OPENMT2001_SAVE_TWO"
                    };
                    let data = std::fs::read(crate::fixtures::path(variable)).unwrap();
                    let save = crate::save::SaveFile::parse(&data).unwrap();
                    let mut clock = save.clock().unwrap();
                    // Controlled 24-hour observation sequence, not save resumption.
                    clock.calendar[0] = 0;
                    loaded = Some((super::Ledgers::from_saved(&save).unwrap(), clock));
                }
                let (ledgers, clock) = loaded.as_mut().unwrap();
                assert_eq!(ledgers.businesses, businesses, "{name} input table");
                let mut controls = crate::clock::Controls {
                    years_per_day: 5,
                    suppress_rollover_query: false,
                    rollover_events_enabled: false,
                    autosave_disabled: true,
                };
                clock.advance_hour(&mut controls, |event, clock, _| {
                    if event == crate::clock::Event::Hourly {
                        assert_eq!(clock.hour(), hour);
                        let mut hourly = crate::hourly::Controls {
                            suppressed: false,
                            world_mode: 3,
                            before_six_sound: 0,
                            network_enabled: true,
                            network_override: false,
                        };
                        // This fixture isolates464100; other hourly callees
                        // are seams. The combined midnight/morning fixture
                        // below also executes the recovered ledger bodies.
                        crate::hourly::advance(
                            clock,
                            &mut hourly,
                            ledgers,
                            |event, clock, _, ledgers| {
                                if event == crate::hourly::Event::Businesses {
                                    ledgers.update_hourly_businesses(clock.hour());
                                }
                                Ok(false)
                            },
                        )
                        .unwrap();
                    }
                    false
                });
                businesses.clone_from(&ledgers.businesses);
            }
            for (index, record) in businesses.iter().enumerate() {
                assert_eq!(
                    record.as_slice(),
                    &bytes[at..at + 544],
                    "{name} business{index}"
                );
                at += 544;
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original combined hourly ledger observations"]
    fn matches_native_hourly_ledger_sequence() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_HOURLY_LEDGERS")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTHRLED2");
        assert_eq!(word(12), 0x37f);
        let mut at = 16;
        let buffer = |at: &mut usize| {
            let length = word(*at) as usize;
            *at += 4;
            let result = &bytes[*at..*at + length];
            *at += length;
            result
        };
        for _ in 0..word(8) {
            let name = std::str::from_utf8(buffer(&mut at)).unwrap();
            let counts = bytes[at..at + 45].try_into().unwrap();
            at += 45;
            let businesses = buffer(&mut at)
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            let player_count = word(at);
            at += 4;
            let mut raw_players: Vec<Vec<u8>> = (0..player_count)
                .map(|_| buffer(&mut at).to_vec())
                .collect();
            let mut ledgers = super::Ledgers {
                businesses,
                players: raw_players
                    .iter()
                    .map(|p| {
                        PlayerAccount::from_saved(&crate::save::SavedPlayer {
                            metadata: &[0; 48],
                            bytes: p[4..0x1bc].try_into().unwrap(),
                        })
                        .unwrap()
                    })
                    .collect(),
                residents: super::ResidentAccounts {
                    active_count: 0,
                    nodes: Vec::new(),
                    accounts: Vec::new(),
                },
            };
            let phases = word(at);
            at += 4;
            let mut clock = crate::clock::Clock {
                calendar: [23, 0, 0, 0, 0],
                timing: [0; 6],
            };
            let mut hourly = crate::hourly::Controls {
                suppressed: false,
                world_mode: 3,
                before_six_sound: 0,
                network_enabled: true,
                network_override: false,
            };
            let mut clock_controls = crate::clock::Controls {
                years_per_day: 1,
                suppress_rollover_query: true,
                rollover_events_enabled: false,
                autosave_disabled: true,
            };
            for _ in 0..phases {
                let hour = word(at);
                at += 4;
                clock.calendar[0] = hour - 1;
                clock.advance_hour(&mut clock_controls, |event, clock, _| {
                    if event == crate::clock::Event::Hourly {
                        assert_eq!(clock.hour(), hour);
                        crate::hourly::advance_ledgers(
                            clock,
                            &mut hourly,
                            &mut ledgers,
                            counts,
                            |_, _, _, _| Ok(false),
                        )
                        .unwrap();
                    }
                    false
                });
                assert_eq!(clock.hour(), if hour == 24 { 0 } else { hour });
                let expected = buffer(&mut at);
                assert_eq!(expected.len(), 600 * 544);
                for (index, record) in ledgers.businesses.iter().enumerate() {
                    assert_eq!(
                        record.as_slice(),
                        &expected[index * 544..(index + 1) * 544],
                        "{name} hour{hour} business{index}"
                    );
                }
                for (raw, player) in raw_players.iter_mut().zip(&ledgers.players) {
                    player.write_runtime(raw).unwrap();
                    assert_eq!(
                        raw.as_slice(),
                        buffer(&mut at),
                        "{name} hour{hour} full player prefix"
                    );
                }
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    fn resident_passes_use_signed_count_prefix() {
        let mut pool = super::ResidentAccounts::new_pool();
        // Native0045bd40/80/dd0 ignore markers, including gaps in this prefix.
        pool.nodes[2].mark_allocated();
        pool.active_count = 2;
        assert_eq!(pool.iteration_len().unwrap(), 2);
        assert!(!pool.nodes[0].is_active());
        assert!(pool.nodes[2].is_active());
        for count in [0, 0x8000_0000, u32::MAX] {
            pool.active_count = count;
            assert_eq!(pool.iteration_len().unwrap(), 0);
        }
        pool.active_count = 2001;
        assert!(pool.iteration_len().is_err());
    }

    #[test]
    #[ignore = "requires original resident-slot observations"]
    fn matches_native_resident_slots() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_SLOTS")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRSLOTS");
        assert_eq!(word(8), 8);
        let mut at = 12;
        for case in 0..word(8) {
            let operation = word(at);
            let expected = [word(at + 4), word(at + 8)];
            at += 12;
            let mut residents = super::ResidentAccounts {
                active_count: 0,
                accounts: vec![],
                nodes: bytes[at..at + 2000 * 0x150]
                    .chunks_exact(0x150)
                    .map(|row| {
                        crate::resident::Resident::from_pool_bytes(
                            row[..0x14a].try_into().unwrap(),
                            row[0x14a],
                        )
                    })
                    .collect(),
            };
            at += 2000 * 0x150;
            if operation == 0 {
                residents.reset_resident_slots().unwrap();
            } else {
                let first = residents
                    .allocate_resident_slot()
                    .unwrap()
                    .map_or(u32::MAX, |i| i as u32);
                assert_eq!(first, expected[0], "case {case} first result");
                if operation == 2 {
                    let second = residents
                        .allocate_resident_slot()
                        .unwrap()
                        .map_or(u32::MAX, |i| i as u32);
                    assert_eq!(second, expected[1], "case {case} second result");
                }
            }
            for (index, node) in residents.nodes.iter().enumerate() {
                let start = at + index * 0x150;
                assert_eq!(
                    node.as_bytes().as_slice(),
                    &bytes[start..start + 0x14a],
                    "case {case} resident {index}"
                );
                assert_eq!(
                    node.active_byte(),
                    bytes[start + 0x14a],
                    "case {case} active {index}"
                );
            }
            at += 2000 * 0x150;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident account-pool observations"]
    fn matches_native_resident_account_pool() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_ACCOUNT_POOL")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTACPOOL");
        assert_eq!(word(8), 8);
        let mut at = 12;
        for case in 0..word(8) {
            let operation = word(at);
            let expected = [word(at + 4), word(at + 8)];
            at += 12;
            let mut residents = super::ResidentAccounts {
                active_count: 0,
                nodes: vec![],
                accounts: bytes[at..at + 2000 * 0x90]
                    .chunks_exact(0x90)
                    .map(|row| {
                        crate::resident::SpendingAccount::from_bytes(row.try_into().unwrap())
                    })
                    .collect(),
            };
            at += 2000 * 0x90;
            if operation == 0 {
                residents.reset_account_pool().unwrap();
            } else {
                let first = residents
                    .allocate_account()
                    .unwrap()
                    .map_or(u32::MAX, |i| i as u32);
                assert_eq!(first, expected[0], "case {case} first result");
                if operation == 2 {
                    let second = residents
                        .allocate_account()
                        .unwrap()
                        .map_or(u32::MAX, |i| i as u32);
                    assert_eq!(second, expected[1], "case {case} second result");
                }
            }
            for (index, account) in residents.accounts.iter().enumerate() {
                let start = at + index * 0x90;
                assert_eq!(
                    account.as_bytes().as_slice(),
                    &bytes[start..start + 0x90],
                    "case {case} account {index}"
                );
            }
            at += 2000 * 0x90;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires native housing append/unlink observations"]
    fn matches_native_housing_links() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_HOUSING_LINKS")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTHLINK1");
        assert_eq!((word(8), word(12)), (18, 4));
        let mut at = 16;
        for case in 0..word(8) {
            let op = word(at);
            let target = word(at + 4) as usize;
            let owner_present = word(at + 8) != 0;
            let success = word(at + 12) != 0;
            let counter = word(at + 16) as i32;
            let expected_counter = word(at + 20) as i32;
            let before_count = word(at + 24) as usize;
            let after_count = word(at + 28) as usize;
            at += 32;
            let mut available: Vec<_> = (0..before_count)
                .map(|i| word(at + i * 4) as usize)
                .collect();
            at += before_count * 4;
            let expected_available: Vec<_> = (0..after_count)
                .map(|i| word(at + i * 4) as usize)
                .collect();
            at += after_count * 4;
            let mut record: [u8; 0x220] = bytes[at..at + 0x220].try_into().unwrap();
            let expected_record = &bytes[at + 0x220..at + 0x440];
            at += 0x440;
            let nodes = bytes[at..at + 4 * 0x14a]
                .chunks_exact(0x14a)
                .map(|row| crate::resident::Resident::from_bytes(row.try_into().unwrap()))
                .collect();
            at += 4 * 0x14a;
            let expected_nodes = &bytes[at..at + 4 * 0x14a];
            at += 4 * 0x14a;
            let mut residents = ResidentAccounts {
                active_count: 4,
                nodes,
                accounts: vec![],
            };
            let mut owner = PlayerAccount {
                owned_categories: [0; 52],
                controller_kind: 0,
                cash: 123.,
                replenishment_modifier: 17,
                retail_counters: [23, 29],
                housing_counter: counter,
                ledgers: [[37.; 38]; 2],
                period: 1,
            };
            let result = if op == 0 {
                residents.append_housing_resident(&mut record, 2, target, &mut available)
            } else {
                residents.remove_housing_resident(
                    &mut record,
                    2,
                    target,
                    &mut available,
                    owner_present.then_some(&mut owner),
                )
            }
            .unwrap();
            assert_eq!(result, success, "return case {case}");
            assert_eq!(&record[..], expected_record, "business case {case}");
            for (node, expected) in residents
                .nodes
                .iter()
                .zip(expected_nodes.chunks_exact(0x14a))
            {
                assert_eq!(node.as_bytes(), expected, "resident case {case}");
            }
            assert_eq!(available, expected_available, "availability case {case}");
            assert_eq!(owner.housing_counter, expected_counter, "owner case {case}");
            assert_eq!(
                (
                    owner.cash,
                    owner.replenishment_modifier,
                    owner.retail_counters,
                    owner.ledgers,
                    owner.period
                ),
                (123., 17, [23, 29], [[37.; 38]; 2], 1)
            );
        }
        assert_eq!(at, bytes.len());
    }

    use super::*;

    fn daily_test_players() -> Vec<PlayerAccount> {
        (0..2)
            .map(|_| {
                let mut player = PlayerAccount::from_prefix(&[0; 0x1b4]).unwrap();
                player.cash = 1000.;
                player
            })
            .collect()
    }

    fn daily_test_residents() -> ResidentAccounts {
        ResidentAccounts {
            active_count: 0,
            nodes: Vec::new(),
            accounts: Vec::new(),
        }
    }

    fn daily_rent_record(owner: u8, rent: u16) -> [u8; 0x220] {
        let mut record = [0; 0x220];
        record[..4].copy_from_slice(&2u32.to_le_bytes());
        record[10] = owner;
        record[0x18..0x1a].copy_from_slice(&rent.to_le_bytes());
        record[0x21c] = 1;
        record
    }

    fn daily_test_parameters(landlord: u8) -> BusinessDayParameters {
        BusinessDayParameters {
            capacity: 100,
            catalog_mask: 0,
            prices: Vec::new(),
            utility_scalar: 1,
            utility_rates: [0.; 4],
            landlord,
        }
    }

    #[test]
    fn daily_rent_postings_keep_payments_separate_and_do_not_post_twice() {
        let mut records = vec![[0; 0x220]; 600];
        let mut parameters: Vec<_> = (0..600).map(|_| None).collect();
        for (slot, owner, landlord, rent) in [(2, 1, 2, 75), (7, 2, 1, 125), (20, 1, 2, 50)] {
            records[slot] = daily_rent_record(owner, rent);
            parameters[slot] = Some(daily_test_parameters(landlord));
        }
        let mut players = daily_test_players();
        let postings = update_daily_businesses(
            &mut records,
            &parameters,
            &mut daily_test_residents(),
            &mut players,
            &mut Utilities::new([0; 4]),
        )
        .unwrap();
        let expected: Vec<_> = [
            (0, -75.),
            (1, 75.),
            (0, -0.),
            (0, -0.),
            (0, -0.),
            (0, -0.),
            (1, -125.),
            (0, 125.),
            (1, -0.),
            (1, -0.),
            (1, -0.),
            (1, -0.),
            (0, -50.),
            (1, 50.),
            (0, -0.),
            (0, -0.),
            (0, -0.),
            (0, -0.),
        ]
        .map(|(player, amount)| AccountPosting { player, amount })
        .into();
        assert_eq!(postings, expected);
        // Main's accounting pays 75+50 and receives 125 for player 0;
        // player 1 does the reverse. Notifications must never reapply cash.
        assert_eq!(
            players.iter().map(|p| p.cash).collect::<Vec<_>>(),
            [1000., 1000.]
        );
        assert_eq!(players[0].ledger(0x0e), Some(-125.));
        assert_eq!(players[0].ledger(0x22), Some(125.));
        assert_eq!(players[1].ledger(0x0e), Some(-125.));
        assert_eq!(players[1].ledger(0x22), Some(125.));
        for slot in [2, 7, 20] {
            let rent = f32::from(u16::from_le_bytes(
                records[slot][0x18..0x1a].try_into().unwrap(),
            ));
            for at in [0x1c, 0x28] {
                assert_eq!(
                    f32::from_le_bytes(records[slot][at..at + 4].try_into().unwrap()),
                    -rent
                );
            }
        }
    }

    #[test]
    fn daily_rent_same_owner_zero_and_unowned_accounts() {
        for (tenant, landlord, rent, expected_rent, expected_cash) in [
            (1, 1, 90, vec![], [1000., 1000.]),
            (1, 2, 0, vec![(0, -0.), (1, 0.)], [1000., 1000.]),
            (0, 1, 20, vec![(0, 20.)], [1020., 1000.]),
            (2, 0, 30, vec![(1, -30.)], [1000., 970.]),
            (0, 0, 99, vec![], [1000., 1000.]),
        ] {
            let mut records = vec![[0; 0x220]; 600];
            records[10] = daily_rent_record(tenant, rent);
            // Inactive entries must never post, even with nonzero rent.
            records[0] = daily_rent_record(1, 500);
            records[0][0x21c] = 0;
            let mut parameters: Vec<_> = (0..600).map(|_| None).collect();
            parameters[10] = Some(daily_test_parameters(landlord));
            let mut players = daily_test_players();
            let postings = update_daily_businesses(
                &mut records,
                &parameters,
                &mut daily_test_residents(),
                &mut players,
                &mut Utilities::new([0; 4]),
            )
            .unwrap();
            let mut expected: Vec<_> = expected_rent
                .into_iter()
                .map(|(player, amount)| AccountPosting { player, amount })
                .collect();
            if tenant != 0 {
                expected.extend(
                    [AccountPosting {
                        player: usize::from(tenant) - 1,
                        amount: -0.,
                    }; 4],
                );
            }
            assert_eq!(postings, expected, "tenant={tenant}, landlord={landlord}");
            assert_eq!(
                players.iter().map(|p| p.cash).collect::<Vec<_>>(),
                expected_cash
            );
        }
    }

    #[test]
    fn daily_postings_include_restock_housing_and_utilities_in_order() {
        let mut records = vec![[0; 0x220]; 600];
        let mut parameters: Vec<_> = (0..600).map(|_| None).collect();
        records[0] = daily_rent_record(1, 75);
        records[0][..4].copy_from_slice(&0u32.to_le_bytes());
        records[0][0x80..0x82].copy_from_slice(&10u16.to_le_bytes());
        records[0][0x1b8..0x1ba].copy_from_slice(&20i16.to_le_bytes());
        let mut retail = daily_test_parameters(2);
        retail.prices = vec![2.];
        retail.utility_rates = [1., 2., 3., 4.];
        parameters[0] = Some(retail);
        records[1] = daily_rent_record(2, 0);
        records[1][..4].copy_from_slice(&1u32.to_le_bytes());
        records[1][0x20c..0x210].copy_from_slice(&(-1i32).to_le_bytes());
        parameters[1] = Some(daily_test_parameters(2));
        let mut players = daily_test_players();
        let postings = update_daily_businesses(
            &mut records,
            &parameters,
            &mut daily_test_residents(),
            &mut players,
            &mut Utilities::new([0; 4]),
        )
        .unwrap();
        let expected: Vec<_> = [
            (0, -30.),
            (0, -75.),
            (1, 75.),
            (0, -1.),
            (0, -2.),
            (0, -3.),
            (0, -4.),
            (1, 0.),
            (1, -0.),
            (1, -0.),
            (1, -0.),
            (1, -0.),
        ]
        .map(|(player, amount)| AccountPosting { player, amount })
        .into();
        assert_eq!(postings, expected);
        assert_eq!(
            players.iter().map(|p| p.cash).collect::<Vec<_>>(),
            [885., 1075.]
        );
        // Retail turnover is a ledger-only record, never a cash notification.
        assert_eq!(players[0].ledger(0x13), Some(-10.));
    }

    #[test]
    #[ignore = "requires original-instruction retail fixture"]
    fn matches_native_retail_purchase() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RETAIL")).unwrap();
        assert_eq!(&bytes[..8], b"MTBUY002");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 0x37f);
        let size = 16 + 2 * (0x220 + 0x300);
        assert_eq!(bytes.len(), 16 + count * size);
        assert_eq!(count, 78);
        for (case, r) in bytes[16..].chunks_exact(size).enumerate() {
            let word = |at| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
            let mut business: [u8; 0x220] = r[16..16 + 0x220].try_into().unwrap();
            let mut player_bytes: [u8; 0x300] = r[16 + 0x220..16 + 0x520].try_into().unwrap();
            let mut player = PlayerAccount::from_saved(&crate::save::SavedPlayer {
                metadata: &[0; 0x30],
                bytes: player_bytes[4..4 + 0x1b8].try_into().unwrap(),
            })
            .unwrap();
            purchase_business_record(
                &mut business,
                0,
                word(0),
                f32::from_bits(word(12)),
                word(4) as i32,
                word(8),
                Some(&mut player),
            )
            .unwrap();
            player_bytes[4..8].copy_from_slice(&player.cash.to_le_bytes());
            for (period, bank) in player.ledgers.iter().enumerate() {
                for (event, value) in bank.iter().enumerate() {
                    let at = 0x14 + (period * 38 + event) * 4;
                    player_bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
            for (at, value) in [0x1ac, 0x1b4].into_iter().zip(player.retail_counters) {
                player_bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
            let expected = &r[16 + 0x520..];
            assert_eq!(
                business.as_slice(),
                &expected[..0x220],
                "business case {case}"
            );
            assert_eq!(
                player_bytes.as_slice(),
                &expected[0x220..],
                "player case {case}"
            );
        }
    }

    #[test]
    #[ignore = "requires original daily rent observations"]
    fn matches_native_rent() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RENT")).unwrap();
        assert_eq!(&bytes[..8], b"MTRNT001");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 7);
        assert_eq!(bytes.len(), 12 + 7 * 84);
        for (case, r) in bytes[12..].chunks_exact(84).enumerate() {
            let word = |i| u32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap());
            let f = |i| f32::from_bits(word(i));
            let mut business = BusinessAccount {
                balance: f(3),
                cost_balance: f(7),
            };
            let mut rent_balance = f(5);
            let mut players: Vec<_> = (0..2)
                .map(|i| {
                    let mut player = PlayerAccount {
                        owned_categories: [0; 52],
                        controller_kind: 0,
                        retail_counters: [0; 2],
                        housing_counter: 0,
                        cash: f(9 + i * 6),
                        replenishment_modifier: 0,
                        ledgers: [[0.; 38]; 2],
                        period: 0,
                    };
                    player.ledgers[0][14] = f(11 + i * 6);
                    player.ledgers[0][34] = f(13 + i * 6);
                    player
                })
                .collect();
            business
                .settle_rent(
                    &mut rent_balance,
                    word(2) as u16,
                    word(0) as u8,
                    word(1) as u8,
                    &mut players,
                )
                .unwrap();
            assert_eq!(business.balance.to_bits(), word(4), "case {case} balance");
            assert_eq!(rent_balance.to_bits(), word(6), "case {case} rent");
            assert_eq!(business.cost_balance.to_bits(), word(8), "case {case} cost");
            for (i, p) in players.iter().enumerate() {
                assert_eq!(p.cash.to_bits(), word(10 + i * 6));
                assert_eq!(p.ledger(14).unwrap().to_bits(), word(12 + i * 6));
                assert_eq!(p.ledger(34).unwrap().to_bits(), word(14 + i * 6));
            }
        }
    }

    #[test]
    #[ignore = "requires original business history observations"]
    fn matches_native_business_history() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_HISTORY")).unwrap();
        assert_eq!(&bytes[..8], b"MTBHIST2");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 0x37f);
        assert_eq!(bytes.len(), 16 + count * (4 + 0x440));
        for (case, r) in bytes[16..].chunks_exact(4 + 0x440).enumerate() {
            let count = u32::from_le_bytes(r[..4].try_into().unwrap()) as u8;
            let mut record: [u8; 0x220] = r[4..4 + 0x220].try_into().unwrap();
            rotate_business_history(&mut record, count).unwrap();
            assert_eq!(
                record.as_slice(),
                &r[4 + 0x220..],
                "whole business record case {case}"
            );
        }
    }

    #[test]
    #[ignore = "requires original owner payout observations"]
    fn matches_native_owner_payouts() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PAYOUTS")).unwrap();
        assert_eq!(&bytes[..8], b"MTPAYOUT");
        assert_eq!(bytes.len(), 12 + 4 * 116);
        for r in bytes[12..].chunks_exact(116) {
            let word = |i| u32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap());
            let mut players: Vec<_> = (0..4)
                .map(|i| PlayerAccount {
                    owned_categories: [0; 52],
                    controller_kind: 0,
                    retail_counters: [0; 2],
                    housing_counter: 0,
                    cash: 100.25 + i as f32,
                    replenishment_modifier: 0,
                    ledgers: [[0.; 38]; 2],
                    period: 0,
                })
                .collect();
            let mut utilities =
                Utilities::new([word(3) as u8, word(2) as u8, word(0) as u8, word(1) as u8]);
            utilities.totals = [7, 6, 4, 5].map(|i| f32::from_bits(word(i)));
            utilities
                .pay_owners(&mut players, f32::from_bits(word(8)))
                .unwrap();
            for (i, player) in players.iter().enumerate() {
                assert_eq!(player.cash.to_bits(), word(9 + i * 5));
                for event in 0..4 {
                    assert_eq!(
                        player.ledger(25 + event).unwrap().to_bits(),
                        word(10 + i * 5 + event)
                    );
                }
            }
            assert_eq!(utilities.totals, [0.; 4]);
        }
    }

    #[test]
    #[ignore = "requires original property-income payout observations"]
    fn matches_native_property_income() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PROPERTY_INCOME")).unwrap();
        assert_eq!(&bytes[..8], b"MTPROPIN");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 4);
        assert_eq!(bytes.len(), 12 + 4 * 116);
        for r in bytes[12..].chunks_exact(116) {
            let word = |i| u32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap());
            let mut players: Vec<_> = (0..4)
                .map(|i| PlayerAccount {
                    owned_categories: [0; 52],
                    controller_kind: 0,
                    retail_counters: [0; 2],
                    housing_counter: 0,
                    cash: 100.25 + i as f32,
                    replenishment_modifier: 0,
                    ledgers: [[0.; 38]; 2],
                    period: 0,
                })
                .collect();
            let mut income = PropertyIncome {
                owners: [0, 1, 2, 3].map(|i| word(i) as u8),
                totals: [4, 5, 6, 7].map(|i| f32::from_bits(word(i))),
            };
            income
                .pay_owners(&mut players, f32::from_bits(word(8)), 7.)
                .unwrap();
            for (i, player) in players.iter().enumerate() {
                assert_eq!(player.cash.to_bits(), word(9 + i * 5));
                for event in 0..4 {
                    assert_eq!(
                        player.ledger(29 + event).unwrap().to_bits(),
                        word(10 + i * 5 + event)
                    );
                }
            }
            assert_eq!(income.totals, [7.; 4]);
        }
    }

    #[test]
    fn combined_owner_payout_native_probe() {
        // Original 0048e070; utility native totals [12.125,6.3,18.7,3.15],
        // all utility owner1; property owners [1,2,1,0], each total800.
        let mut players: Vec<_> = (0..2)
            .map(|i| PlayerAccount {
                owned_categories: [0; 52],
                controller_kind: 0,
                retail_counters: [0; 2],
                housing_counter: 0,
                cash: 100.25 + i as f32,
                replenishment_modifier: 0,
                ledgers: [[0.; 38]; 2],
                period: 0,
            })
            .collect();
        let mut utilities = Utilities::new([1; 4]);
        utilities.totals = [3.15, 18.7, 12.125, 6.3];
        utilities.enabled = [false; 4];
        let mut properties = PropertyIncome {
            owners: [1, 2, 1, 0],
            totals: [800.; 4],
        };
        OwnerIncomeSettings::original_hub(1)
            .unwrap()
            .pay_owners(&mut utilities, &mut properties, &mut players)
            .unwrap();
        assert_eq!(players[0].cash.to_bits(), 0x446b219a);
        assert_eq!(players[1].cash.to_bits(), 0x4396a000);
        assert_eq!(utilities.totals, [0.; 4]);
        assert_eq!(properties.totals, [800.; 4]);
        assert_eq!(utilities.enabled, [true; 4]);
    }

    #[test]
    fn utility_owner_payout_native_probe() {
        // Original 0048dfc0 and 0048a5a0 executed with four owned services,
        // original index totals [12,6,18,3], factor1 and starting cash100.
        let mut players = [PlayerAccount {
            owned_categories: [0; 52],
            controller_kind: 0,
            retail_counters: [0; 2],
            housing_counter: 0,
            cash: 100.,
            replenishment_modifier: 0,
            ledgers: [[0.; 38]; 2],
            period: 0,
        }];
        let mut utilities = Utilities::new([1; 4]);
        utilities.totals = [3., 18., 12., 6.];
        utilities.pay_owners(&mut players, 1.).unwrap();
        assert_eq!(players[0].cash, 139.);
        assert_eq!(
            [25, 26, 27, 28].map(|i| players[0].ledger(i).unwrap()),
            [12., 6., 18., 3.]
        );
        assert_eq!(utilities.totals, [0.; 4]);
    }

    #[test]
    #[ignore = "requires original utility observations and supplied save"]
    fn matches_native_utilities() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_UTILITIES")).unwrap();
        let saved = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&saved).unwrap();
        let economy = save.economy().unwrap();
        let buildings = save.buildings().unwrap();
        assert_eq!(&bytes[..8], b"MTUTIL01");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 14);
        assert_eq!(bytes.len(), 12 + 14 * 128);
        for r in bytes[12..].chunks_exact(128) {
            let word = |i| u32::from_le_bytes(r[i * 4..i * 4 + 4].try_into().unwrap());
            let float = |i| f32::from_bits(word(i));
            let building = buildings
                .iter()
                .find(|b| b.slot == word(1) as usize)
                .unwrap();
            let [x0, y0, x1, y1] = building.lot_endpoints().map(i32::from);
            let scalar = (x1 - x0 + 1) * (y1 - y0 + 1) * i32::from(building.height());
            let mut utilities = Utilities {
                owners: std::array::from_fn(|i| if word(3) == i as u32 { 1 } else { 127 }),
                enabled: std::array::from_fn(|i| word(2) & (1 << i) != 0),
                totals: [float(17), float(16), float(14), float(15)],
            };
            let mut owner = PlayerAccount {
                owned_categories: [0; 52],
                controller_kind: 0,
                retail_counters: [0; 2],
                housing_counter: 0,
                cash: float(9),
                replenishment_modifier: 0,
                ledgers: [[0.; 38]; 2],
                period: 0,
            };
            for (i, event) in [7, 6, 4, 5].into_iter().enumerate() {
                owner.ledgers[0][event] = float(10 + i);
            }
            let mut business = BusinessAccount {
                balance: float(8),
                cost_balance: 0.,
            };
            let mut expenses = std::array::from_fn(|i| float(4 + i));
            let amounts = utilities.assess(scalar, economy.utility_rates(word(0)).unwrap(), 1);
            business
                .charge_utilities(&mut expenses, amounts, true, Some(&mut owner))
                .unwrap();
            for i in 0..4 {
                assert_eq!(expenses[i].to_bits(), word(18 + i));
                assert_eq!(
                    owner.ledger([7, 6, 4, 5][i]).unwrap().to_bits(),
                    word(24 + i)
                );
                assert_eq!(utilities.totals[i].to_bits(), word(28 + [3, 2, 0, 1][i]));
            }
            assert_eq!(business.balance.to_bits(), word(22));
            assert_eq!(owner.cash.to_bits(), word(23));
        }
    }

    #[test]
    #[ignore = "requires supplied saved player for native rotation comparison"]
    fn saved_player_ledger_rotation() {
        let bytes = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&bytes).unwrap();
        let players = save.players().unwrap();
        let mut account = PlayerAccount::from_saved(&players[0]).unwrap();
        let original = account.ledgers.map(|bank| bank.map(f32::to_bits));
        assert_eq!(account.period, 1);
        // Original 0048c9b0 observed twice in native_history_rotation.py.
        account.rotate_ledger();
        assert_eq!(account.period, 0);
        assert_eq!(account.ledgers[0].map(f32::to_bits), [0; 38]);
        assert_eq!(account.ledgers[1].map(f32::to_bits), original[1]);
        assert_eq!(account.cash.to_bits(), 0x46216168);
        account.rotate_ledger();
        assert_eq!(account.period, 1);
        assert_eq!(
            account.ledgers.map(|bank| bank.map(f32::to_bits)),
            [[0; 38]; 2]
        );
        assert_eq!(account.cash.to_bits(), 0x46216168);
    }

    #[test]
    #[ignore = "requires save-backed original replenishment observations"]
    fn matches_saved_replenishment_accounting() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_SAVED_REPLENISHMENT")).unwrap();
        let mut at = 0;
        let mut cases = 0;
        while at < bytes.len() {
            assert_eq!(&bytes[at..at + 8], b"MTNSBIN1");
            let header = |offset| {
                u32::from_le_bytes(bytes[at + offset..at + offset + 4].try_into().unwrap())
            };
            assert_eq!(header(8), 1);
            assert_eq!(header(16), 192);
            let count = header(12);
            at += 88;
            for _ in 0..count {
                let r = &bytes[at..at + 192];
                let word = |offset| u32::from_le_bytes(r[offset..offset + 4].try_into().unwrap());
                let short = |offset| u16::from_le_bytes(r[offset..offset + 2].try_into().unwrap());
                let float = |offset| f32::from_bits(word(offset));
                let mut account = PlayerAccount {
                    owned_categories: [0; 52],
                    controller_kind: 0,
                    retail_counters: [0; 2],
                    housing_counter: 0,
                    cash: float(168),
                    replenishment_modifier: word(20),
                    ledgers: [[0.; 38]; 2],
                    period: 0,
                };
                account.ledgers[0][0x11] = float(176);
                account.ledgers[0][0x13] = float(184);
                let mut business = BusinessAccount {
                    cost_balance: float(152),
                    balance: float(160),
                };
                let mut lines: Vec<_> = (0..word(32) as usize)
                    .map(|i| RestockLine {
                        stock: short(80 + i * 2),
                        target: short(92 + i * 2) as i16,
                        unit_cost: float(104 + i * 4),
                    })
                    .collect();
                let capacity = stock_capacity(word(36) as i32, word(40) as u8, float(48));
                let result = business
                    .replenish(&mut lines, capacity, word(16) != 0, Some(&mut account))
                    .unwrap();
                for (i, slot) in result.slots.iter().enumerate() {
                    assert_eq!(lines[i].stock, short(128 + i * 2));
                    assert_eq!(slot.turnover, short(140 + i * 2));
                }
                assert_eq!(
                    business.cost_balance.to_bits(),
                    word(156),
                    "cost case {cases}"
                );
                assert_eq!(
                    business.balance.to_bits(),
                    word(164),
                    "balance case {cases}"
                );
                assert_eq!(account.cash.to_bits(), word(172), "cash case {cases}");
                assert_eq!(account.ledger(0x11).unwrap().to_bits(), word(180));
                assert_eq!(account.ledger(0x13).unwrap().to_bits(), word(188));
                at += 192;
                cases += 1;
            }
        }
        assert_eq!(cases, 24);
    }

    #[test]
    #[ignore = "requires supplied save and original housing observations"]
    fn matches_native_housing_records() {
        let source = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&source).unwrap();
        let saved_residents = save.residents().unwrap();
        let economy = save.economy().unwrap();
        let businesses: Vec<_> = economy.businesses().collect();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_HOUSING")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTHOUS01");
        let mut at = 12;
        for case in 0..word(8) {
            let slot = word(at) as usize;
            let tenant = word(at + 4) as u8;
            let landlord = word(at + 8) as u8;
            let mut record = *businesses[slot].bytes;
            record[10] = tenant;
            let mut players = vec![PlayerAccount {
                owned_categories: [0; 52],
                controller_kind: 0,
                retail_counters: [0; 2],
                housing_counter: 0,
                cash: f32::from_bits(word(at + 12)),
                replenishment_modifier: 0,
                ledgers: [[0.; 38]; 2],
                period: 0,
            }];
            for (i, event) in [14, 33, 34].into_iter().enumerate() {
                players[0].ledgers[0][event] = f32::from_bits(word(at + 16 + i * 4));
            }
            let mut residents = ResidentAccounts::from_saved(&saved_residents);
            let owner = if tenant == 0 {
                None
            } else {
                Some(&mut players[0])
            };
            residents
                .collect_housing_income(&mut record, owner)
                .unwrap();
            let float = |offset| f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
            let mut account = BusinessAccount {
                cost_balance: float(0x30),
                balance: float(0x1c),
            };
            let mut rent_balance = float(0x28);
            let rent = u16::from_le_bytes(record[0x18..0x1a].try_into().unwrap());
            account
                .settle_rent(&mut rent_balance, rent, tenant, landlord, &mut players)
                .unwrap();
            record[0x1c..0x20].copy_from_slice(&account.balance.to_le_bytes());
            record[0x28..0x2c].copy_from_slice(&rent_balance.to_le_bytes());
            assert_eq!(players[0].cash.to_bits(), word(at + 28));
            for (i, event) in [14, 33, 34].into_iter().enumerate() {
                assert_eq!(
                    players[0].ledger(event).unwrap().to_bits(),
                    word(at + 32 + i * 4)
                );
            }
            at += 44;
            assert_eq!(
                record.as_slice(),
                &bytes[at..at + 0x220],
                "housing business {case}"
            );
            at += 0x220;
            assert_eq!(residents.accounts.len(), 2000);
            for (i, account) in residents.accounts.iter().enumerate() {
                assert_eq!(
                    account.balance().to_bits(),
                    word(at + i * 4),
                    "housing case{case} resident{i}"
                );
            }
            at += 2000 * 4;
        }
        assert_eq!(at, bytes.len());
        assert_eq!(word(8), 11);
    }

    #[test]
    #[ignore = "requires supplied save and original phase-six accounting observations"]
    fn matches_native_phase_six_accounting() {
        check_phase_six(false);
    }

    #[test]
    #[ignore = "requires original hourly finance observations"]
    fn matches_native_hourly_finance() {
        check_phase_six(true);
    }

    fn check_phase_six(hourly: bool) {
        let source = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&source).unwrap();
        let saved_economy = save.economy().unwrap();
        let buildings = save.buildings().unwrap();
        let bytes = std::fs::read(crate::fixtures::path(if hourly {
            "OPENMT2001_NATIVE_HOURLY_FINANCE"
        } else {
            "OPENMT2001_NATIVE_PHASE_SIX"
        }))
        .unwrap();
        assert_eq!(&bytes[..8], if hourly { b"MTHFIN03" } else { b"MTDAY007" });
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 0x37f);
        let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let text = |key| String::from_utf8(read(key)).unwrap();
        let shopping_settings = crate::shopping::Settings::from_saved(
            &save,
            &image,
            &text("OPENMT2001_DEFAULT_SETTINGS"),
            &text("OPENMT2001_BUSINESS_SETTINGS"),
            &text("OPENMT2001_PARAMETERS"),
        )
        .unwrap();
        let business_settings = saved_economy.daily_settings();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let settings = OwnerIncomeSettings::original_hub(save.difficulty().unwrap()).unwrap();
        let mut at = 16;
        for case in 0..word(8) {
            let category_owners: [u8; 52] = bytes[at..at + 52].try_into().unwrap();
            at += 52;
            let modifier = word(at);
            at += 4;
            let gates = std::array::from_fn(|i| bytes[at + i] != 0);
            at += 4;
            let mut utilities = Utilities {
                owners: [2, 35, 39, 42].map(|i| category_owners[i]),
                enabled: [false; 4],
                totals: [123.; 4],
            };
            let mut properties = PropertyIncome {
                owners: [4, 17, 43, 44].map(|i| category_owners[i]),
                totals: [321.; 4],
            };
            settings.reset_income(&mut utilities, &mut properties);
            let mut reset = Vec::new();
            for amount in [2, 3, 1, 0]
                .map(|i| utilities.totals[i])
                .into_iter()
                .chain(properties.totals)
            {
                reset.extend(amount.to_le_bytes());
            }
            reset.extend(utilities.enabled.map(u8::from));
            assert_eq!(reset, &bytes[at..at + 36], "load reset case{case}");
            at += 36;
            utilities.enabled = gates;
            let parameters = saved_economy
                .daily_business_parameters(&buildings, &category_owners)
                .unwrap();
            let mut records: Vec<_> = saved_economy.businesses().map(|b| *b.bytes).collect();
            let mut residents = ResidentAccounts::from_saved(&save.residents().unwrap());
            let mut players: Vec<_> = save
                .players()
                .unwrap()
                .iter()
                .map(|p| PlayerAccount::from_saved(p).unwrap())
                .collect();
            players[0].replenishment_modifier = modifier;
            let mut hourly_result = Vec::new();
            if hourly {
                let mut topology =
                    crate::shopping::Topology::from_saved(&save, &board, &[[0.; 3]; 600]).unwrap();
                for (category, owner) in topology.categories.iter_mut().zip(category_owners) {
                    category.owner = owner;
                    category.countdowns = [0; 2];
                }
                if case == 4 {
                    topology.categories[36].countdowns[0] = 1;
                    players[0].owned_categories[36] = 1;
                }
                let mut prefixes: Vec<[u8; 64]> = save
                    .building_prefixes()
                    .unwrap()
                    .chunks_exact(64)
                    .map(|p| p.try_into().unwrap())
                    .collect();
                let mut ledgers = super::Ledgers {
                    businesses: records,
                    residents,
                    players,
                };
                let mut finance = crate::hourly::Finance {
                    topology: &mut topology,
                    buildings: &mut prefixes,
                    shopping: &shopping_settings,
                    business: &business_settings,
                    utilities: &mut utilities,
                    properties: &mut properties,
                    income: settings,
                };
                let mut clock = save.clock().unwrap();
                clock.calendar[0] = 6;
                let mut controls = crate::hourly::Controls {
                    suppressed: false,
                    world_mode: 3,
                    before_six_sound: 0,
                    network_enabled: true,
                    network_override: false,
                };
                let mut expiry = crate::hourly::CategoryContext {
                    render_kinds: &[1; 600],
                    controls: crate::category::Controls {
                        current_owner_index: 5,
                        selected_category: None,
                        selection_active: false,
                    },
                    normal_tick: 123,
                    accounting_enabled: true,
                    refresh_counter: 0,
                };
                finance
                    .advance(
                        &mut clock,
                        &mut controls,
                        &mut ledgers,
                        |event, _, _, ledgers, finance| {
                            if event == crate::hourly::Event::Categories {
                                finance.expire_categories(
                                    ledgers,
                                    &mut expiry,
                                    |_, _, _, _| Ok(()),
                                    |_, _| Ok(()),
                                )?;
                            }
                            Ok(false)
                        },
                    )
                    .unwrap();
                hourly_result.extend(finance.topology.categories.map(|c| c.owner));
                hourly_result.extend(ledgers.players[0].owned_categories);
                hourly_result.push(expiry.refresh_counter);
                if case == 4 {
                    assert_eq!(finance.topology.categories[36].owner, 0);
                    assert_eq!(expiry.refresh_counter, 100);
                }
                super::Ledgers {
                    businesses: records,
                    residents,
                    players,
                } = ledgers;
            } else {
                settings
                    .settle_day(
                        &mut records,
                        &parameters,
                        &mut residents,
                        &mut players,
                        &mut utilities,
                        &mut properties,
                    )
                    .unwrap();
            }
            let mut actual = Vec::new();
            for record in records {
                actual.extend(record);
            }
            for account in residents.accounts {
                actual.extend(account.balance().to_le_bytes());
            }
            assert_eq!(players.len(), 1);
            let player = &players[0];
            actual.extend(player.cash.to_le_bytes());
            for bank in player.ledgers {
                for value in bank {
                    actual.extend(value.to_le_bytes());
                }
            }
            actual.push(player.period as u8);
            actual.extend(player.replenishment_modifier.to_le_bytes());
            for amount in utilities.totals.into_iter().chain(properties.totals) {
                actual.extend(amount.to_le_bytes());
            }
            actual.extend(utilities.enabled.map(u8::from));
            actual.extend(hourly_result);
            let expected = &bytes[at..at + actual.len()];
            if let Some(i) = actual.iter().zip(expected).position(|(a, b)| a != b) {
                panic!(
                    "phase-six case{case} state byte{i}: Rust={} native={}",
                    actual[i], expected[i]
                );
            }
            at += actual.len();
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires supplied save and original complete business-day observation"]
    fn matches_native_business_day() {
        let source = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&source).unwrap();
        let saved_economy = save.economy().unwrap();
        let buildings = save.buildings().unwrap();
        let category_owners = std::array::from_fn(|i| save.category_owner(i).unwrap());
        let mut records: Vec<_> = saved_economy.businesses().map(|b| *b.bytes).collect();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let defaults = std::fs::read_to_string(crate::fixtures::path("BUSINESS_SETTINGS")).unwrap();
        let catalogs = crate::shopping::Catalogs::from_v14_defaults(&defaults, &image).unwrap();
        catalogs.validate_saved(&saved_economy).unwrap();
        let prefixes: Vec<[u8; 64]> = save
            .building_prefixes()
            .unwrap()
            .chunks_exact(64)
            .map(|p| p.try_into().unwrap())
            .collect();
        let mut categories = [-1; 600];
        for building in &buildings {
            categories[building.slot] = building.category as i16;
        }
        let prices = saved_economy.resident_commodities().map(|c| c.value);
        let settings = saved_economy.daily_settings();
        let parameters = BusinessDayInputs {
            buildings: &prefixes,
            building_categories: &categories,
            category_owners: &category_owners,
            catalogs: &catalogs,
            unit_prices: &prices,
            settings: &settings,
        }
        .parameters(&records)
        .unwrap();
        let mut residents = ResidentAccounts::from_saved(&save.residents().unwrap());
        let mut players: Vec<_> = save
            .players()
            .unwrap()
            .iter()
            .map(|p| PlayerAccount::from_saved(p).unwrap())
            .collect();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_DAY")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTDAY002");
        assert_eq!(word(32), 0x37f);
        let mut utilities = Utilities {
            owners: bytes[8..12].try_into().unwrap(),
            enabled: std::array::from_fn(|i| bytes[12 + i] != 0),
            totals: std::array::from_fn(|i| f32::from_bits(word(16 + i * 4))),
        };
        assert_eq!(utilities.owners, save.utility_owners().unwrap());
        update_daily_businesses(
            &mut records,
            &parameters,
            &mut residents,
            &mut players,
            &mut utilities,
        )
        .unwrap();
        let mut at = 36;
        for (i, record) in records.iter().enumerate() {
            assert_eq!(
                record.as_slice(),
                &bytes[at..at + 0x220],
                "daily business slot{i}"
            );
            at += 0x220;
        }
        for (i, account) in residents.accounts.iter().enumerate() {
            assert_eq!(account.balance().to_bits(), word(at), "daily resident{i}");
            at += 4;
        }
        assert_eq!(players.len(), 1);
        let player = &players[0];
        assert_eq!(player.cash.to_bits(), word(at));
        at += 4;
        for bank in player.ledgers {
            for value in bank {
                assert_eq!(value.to_bits(), word(at), "daily player ledger byte{at}");
                at += 4;
            }
        }
        assert_eq!(player.period, usize::from(bytes[at]));
        at += 1;
        assert_eq!(player.replenishment_modifier, word(at));
        at += 4;
        for amount in utilities.totals {
            assert_eq!(amount.to_bits(), word(at));
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original multi-product current-world daily observations"]
    fn matches_native_current_day_inputs() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_DAY_INPUTS")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTDAYIN2");
        let mut at = 12;
        let buffer = |at: &mut usize| {
            let n = word(*at) as usize;
            *at += 4;
            let b = &bytes[*at..*at + n];
            *at += n;
            b
        };
        for case in 0..word(8) {
            let ids = [word(at), word(at + 4)];
            let prices = [f32::from_bits(word(at + 8)), f32::from_bits(word(at + 12))];
            let mut buildings = vec![[0; 64]; 600];
            for i in 0..5 {
                buildings[0][4 + i] = word(at + 16 + i * 4) as u8;
            }
            let mut owners = [0; 52];
            owners[0] = word(at + 36) as u8;
            at += 40;
            let mut records: Vec<[u8; 544]> = buffer(&mut at)
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            let expected_businesses = buffer(&mut at);
            let mut raw_player = buffer(&mut at).to_vec();
            let expected_player = buffer(&mut at);
            let mut players = vec![
                PlayerAccount::from_saved(&crate::save::SavedPlayer {
                    metadata: &[0; 48],
                    bytes: raw_player[4..0x1bc].try_into().unwrap(),
                })
                .unwrap(),
            ];
            let mut catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| Vec::new()),
                fractions: std::array::from_fn(|_| Vec::new()),
            };
            catalogs.commodities[12] = ids.to_vec();
            let mut unit_prices = [0.; 32];
            for (id, price) in ids.into_iter().zip(prices) {
                unit_prices[id as usize] = price;
            }
            let mut settings = BusinessDaySettings {
                capacity_rates: [0.; 45],
                utility_rates: [[0.; 4]; 45],
                catalog_masks: [0; 45],
            };
            settings.capacity_rates[12] = 1.;
            settings.utility_rates[12] = [0.5, 1., 1.5, 2.];
            settings.catalog_masks[12] = (1 << ids[0]) | (1 << ids[1]);
            let mut categories = [-1; 600];
            categories[0] = 0;
            let parameters = BusinessDayInputs {
                buildings: &buildings,
                building_categories: &categories,
                category_owners: &owners,
                catalogs: &catalogs,
                unit_prices: &unit_prices,
                settings: &settings,
            }
            .parameters(&records)
            .unwrap();
            let mut residents = ResidentAccounts {
                active_count: 0,
                nodes: Vec::new(),
                accounts: Vec::new(),
            };
            let mut utilities = Utilities::new([1; 4]);
            update_daily_businesses(
                &mut records,
                &parameters,
                &mut residents,
                &mut players,
                &mut utilities,
            )
            .unwrap();
            for (slot, record) in records.iter().enumerate() {
                assert_eq!(
                    record.as_slice(),
                    &expected_businesses[slot * 544..(slot + 1) * 544],
                    "case{case} business{slot}"
                );
            }
            raw_player[4..8].copy_from_slice(&players[0].cash.to_le_bytes());
            for (period, bank) in players[0].ledgers.iter().enumerate() {
                for (event, value) in bank.iter().enumerate() {
                    let offset = 0x14 + (period * 38 + event) * 4;
                    raw_player[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
            assert_eq!(raw_player, expected_player, "case{case} landlord prefix");
            assert_eq!(word(at), 0, "native fixture has no function stubs");
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original whole-record replenishment observations"]
    fn matches_replenished_business_records() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_REPLENISHMENT_RECORDS")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTNSBIN1");
        assert_eq!(word(8), 1);
        assert_eq!(word(16), 192);
        let count = word(12) as usize;
        let mut at = 88 + count * 192;
        assert_eq!(&bytes[at..at + 8], b"MTBIZ001");
        assert_eq!(word(at + 8), 1);
        assert_eq!(word(at + 12) as usize, count);
        assert_eq!(word(at + 16), 0x220);
        assert_eq!(word(at + 20), 16);
        at += 24;
        for i in 0..count {
            let input = 88 + i * 192;
            assert_eq!(word(at), word(input));
            assert_eq!(word(at + 8), 0x220);
            assert_eq!(word(at + 12), 0x220);
            let trace_count = word(at + 16) as usize;
            at += 20;
            let mut record: [u8; 0x220] = bytes[at..at + 0x220].try_into().unwrap();
            let float = |offset| f32::from_bits(word(input + offset));
            let mut owner = PlayerAccount {
                owned_categories: [0; 52],
                controller_kind: 0,
                retail_counters: [0; 2],
                housing_counter: 0,
                cash: float(168),
                replenishment_modifier: word(input + 20),
                ledgers: [[0.; 38]; 2],
                period: 0,
            };
            owner.ledgers[0][0x11] = float(176);
            owner.ledgers[0][0x13] = float(184);
            // Supplied records have one product, so catalog order and mask
            // are unambiguous from the fixture's saved input identity.
            assert_eq!(word(input + 32), 1);
            let mask = 1u32 << word(input + 56);
            let capacity =
                stock_capacity(word(input + 36) as i32, word(input + 40) as u8, float(48));
            replenish_business_record(
                &mut record,
                &[float(104)],
                capacity,
                mask,
                word(input + 16) != 0,
                Some(&mut owner),
            )
            .unwrap();
            assert_eq!(
                record.as_slice(),
                &bytes[at + 0x220..at + 0x440],
                "whole business case {i}"
            );
            assert_eq!(owner.cash.to_bits(), word(input + 172));
            assert_eq!(owner.ledger(0x11).unwrap().to_bits(), word(input + 180));
            assert_eq!(owner.ledger(0x13).unwrap().to_bits(), word(input + 188));
            at += 0x440 + trace_count * 16;
        }
        assert_eq!(at, bytes.len());
        assert_eq!(count, 6);
    }

    #[test]
    #[ignore = "requires original-instruction replenishment observations"]
    fn matches_native_replenishment() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_REPLENISHMENT")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTREPLN1");
        let mut at = 12;
        for case in 0..word(8) {
            let count = word(at) as usize;
            let capacity = stock_capacity(1, word(at + 8) as u8, f32::from_bits(word(at + 4)));
            let modifier = if word(at + 20) != 0 { word(at + 12) } else { 0 };
            let charged = word(at + 16) == 0;
            at += 24;
            let mut lines = Vec::new();
            let mut expected = Vec::new();
            for _ in 0..count {
                lines.push(RestockLine {
                    stock: u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()),
                    target: i16::from_le_bytes(bytes[at + 2..at + 4].try_into().unwrap()),
                    unit_cost: f32::from_bits(word(at + 4)),
                });
                expected.push((
                    u16::from_le_bytes(bytes[at + 8..at + 10].try_into().unwrap()),
                    u16::from_le_bytes(bytes[at + 10..at + 12].try_into().unwrap()),
                ));
                at += 12;
            }
            let actual = replenish_stock(&lines, capacity, modifier);
            assert_eq!(
                actual
                    .slots
                    .iter()
                    .map(|s| (s.stock, s.turnover))
                    .collect::<Vec<_>>(),
                expected,
                "case {case}"
            );
            assert_eq!(actual.total_stock, word(at), "case {case} total stock");
            if charged {
                assert_eq!(
                    ((0f64 - actual.cost) as f32).to_bits(),
                    word(at + 4),
                    "case {case} cost"
                );
            }
            at += 8;
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original-instruction restocking observations"]
    fn matches_native_restocking_cost() {
        let native = std::fs::read(crate::fixtures::path("NATIVE_RESTOCK")).unwrap();
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTRESTK1");
        let mut at = 12;
        for case in 0..word(8) {
            let count = word(at) as usize;
            at += 4;
            let mut lines = Vec::new();
            for _ in 0..count {
                lines.push(RestockLine {
                    stock: u16::from_le_bytes(native[at..at + 2].try_into().unwrap()),
                    target: i16::from_le_bytes(native[at + 2..at + 4].try_into().unwrap()),
                    unit_cost: f32::from_bits(word(at + 4)),
                });
                at += 8;
            }
            assert_eq!(
                restocking_cost(&lines).to_bits(),
                word(at),
                "case {case}: {lines:?}"
            );
            at += 4;
        }
        assert_eq!(at, native.len());
    }
}
