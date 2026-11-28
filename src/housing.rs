//! Housing reevaluation over original slot and linked-list order.
use crate::economy::{PlayerAccount, ResidentAccounts};
use crate::resident::{
    DailyObjectives, HousingCandidate, HousingScoreInputs, Welfare, score_housing_category_cached,
    select_residence,
};

/// The housing subset of original CLASSWEIGHTING settings, with image defaults
/// retained for untouched cells. Loading this does not execute the parameter file.
pub struct Settings {
    pub affordability_factor: f32,
    /// VALUE, PRESTIGE, QUALITY in address order (53a6d8/e0/e4).
    pub weights: [[f32; 3]; 3],
}

impl Settings {
    pub fn from_parameters(source: &str, image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let weights = crate::resident::ClassWeights::from_parameters(source, image)?;
        Ok(Self {
            affordability_factor: image.f32_at(0x53a54c)?,
            weights: weights.rows.map(|row| [row[0], row[1], row[2]]),
        })
    }
}

/// Rebuilt building inputs used by original 0045a770 and its callees.
pub struct Building {
    pub business: Option<usize>,
    pub category: i8,
    pub attribute_2c: i8,
    /// Original building +224, copied by 0045cd70 with Y replaced by 0.5.
    pub housing_position: [f32; 3],
}

pub struct Category {
    /// Building slot indices in original next+44 order.
    pub buildings: Vec<usize>,
    pub value: f32,
    /// Runtime +e0/+e4, rebuilt from the original board, outside the save prefix.
    pub center: Option<[f32; 2]>,
    /// Runtime +88, preserved by the save loader and incremented by0045c3d0.
    pub resident_counter: u32,
}

/// Rebuilt housing metadata. Entrance coordinates must come from the original
/// geometry/placement path; they are not inferred from current occupants.
pub struct Topology {
    pub buildings: Vec<Building>,
    pub categories: [Category; 52],
    active: Vec<bool>,
}

impl Topology {
    /// Read the current runtime city, including bound entrance+224 and category
    /// next+44 order. Rebuild after topology changes; this is a derived view,
    /// not an alternative owner of mutable resident/business records.
    pub fn from_runtime(
        records: &[[u8; 0x394]],
        category_records: &[[u8; 0x17c]; 52],
        building_base: u32,
    ) -> Result<Self, String> {
        let word = |bytes: &[u8], at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let chains =
            crate::building_pool::category_chains(category_records, records, building_base)?;
        let mut buildings = Vec::with_capacity(records.len());
        let mut active = Vec::with_capacity(records.len());
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
            buildings.push(Building {
                business,
                category: -1,
                attribute_2c: row[0x2c] as i8,
                housing_position: std::array::from_fn(|axis| {
                    f32::from_bits(word(row, 0x224 + axis * 4))
                }),
            });
            active.push(row[0x3f] != 0);
        }
        let mut categories = std::array::from_fn(|index| Category {
            buildings: chains[index].clone(),
            value: f32::from_bits(word(&category_records[index], 0x84)),
            center: Some([
                f32::from_bits(word(&category_records[index], 0xe0)),
                f32::from_bits(word(&category_records[index], 0xe4)),
            ]),
            resident_counter: word(&category_records[index], 0x88),
        });
        for (index, category) in categories.iter_mut().enumerate() {
            for &slot in &category.buildings {
                if buildings[slot].category != -1 {
                    return Err("duplicate runtime building membership".into());
                }
                buildings[slot].category = index as i8;
            }
        }
        Ok(Self {
            buildings,
            categories,
            active,
        })
    }

    /// Reconstruct residential entrance positions from the user's original
    /// BuildFile, saved footprint/model inputs and recovered board coordinates.
    pub fn reconstruct(
        save: &crate::save::SaveFile<'_>,
        board: &crate::board::Board,
        file: &crate::build_file::BuildFile<'_>,
    ) -> Result<Self, String> {
        let economy = save.economy()?;
        let businesses: Vec<_> = economy.businesses().collect();
        let mut positions = [[0.; 3]; 600];
        for saved in save.buildings()? {
            let business = i32::from_le_bytes(saved.bytes[0x1c..0x20].try_into().unwrap());
            let Some(business) = usize::try_from(business)
                .ok()
                .and_then(|i| businesses.get(i))
            else {
                continue;
            };
            if business.discriminator() != 1 {
                continue;
            }
            let scene = crate::scene::SceneBuilding::from_saved(&saved, board, file)?;
            let category_center = board
                .blocks
                .iter()
                .find(|b| b.id == saved.category)
                .ok_or("saved housing category missing from board")?
                .center;
            positions[saved.slot] = *scene
                .entrance_positions(category_center)?
                .first()
                .ok_or("residential building has no entrance")?;
        }
        let mut topology = Self::from_saved(save, &positions)?;
        for (i, category) in topology.categories.iter_mut().enumerate() {
            // The runtime table has52 slots; not every slot is a map block.
            category.center = board.blocks.iter().find(|b| b.id == i).map(|b| b.center);
        }
        Ok(topology)
    }

    pub fn from_saved(
        save: &crate::save::SaveFile<'_>,
        entrance_positions: &[[f32; 3]; 600],
    ) -> Result<Self, String> {
        let mut buildings = Vec::with_capacity(600);
        let mut active = Vec::with_capacity(600);
        for (slot, prefix) in save.building_prefixes()?.chunks_exact(0x40).enumerate() {
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
            buildings.push(Building {
                business,
                category: -1,
                attribute_2c: prefix[0x2c] as i8,
                housing_position: entrance_positions[slot],
            });
            active.push(prefix[0x3f] != 0);
        }
        let mut categories: [Category; 52] = std::array::from_fn(|_| Category {
            buildings: vec![],
            value: 0.,
            center: None,
            resident_counter: 0,
        });
        for (i, category) in categories.iter_mut().enumerate() {
            category.value = save.category_prestige(i)?;
            category.resident_counter = save.category_resident_counter(i)?;
        }
        for building in save.buildings()? {
            if buildings[building.slot].category != -1 {
                return Err("duplicate saved building membership".into());
            }
            buildings[building.slot].category = building.category as i8;
            categories[building.category].buildings.push(building.slot);
        }
        Ok(Self {
            buildings,
            categories,
            active,
        })
    }

    /// Residential tail of the native city price-cache refresh after load.
    pub fn average_rent(&self, businesses: &[[u8; 0x220]]) -> Result<f32, String> {
        let mut candidates = Vec::new();
        for (building, active) in self.buildings.iter().zip(&self.active) {
            if !active {
                continue;
            }
            let Some(index) = building.business else {
                continue;
            };
            let record = businesses
                .get(index)
                .ok_or("active building business exceeds pool")?;
            candidates.push(HousingCandidate {
                residential: u32::from_le_bytes(record[..4].try_into().unwrap()) == 1,
                capacity: u16::from_le_bytes(record[0xe..0x10].try_into().unwrap()),
                occupied: u16::from_le_bytes(record[0x10..0x12].try_into().unwrap()),
                attribute_2c: building.attribute_2c,
                rent: f32::from_le_bytes(record[0x1dc..0x1e0].try_into().unwrap()),
            });
        }
        Ok(crate::resident::average_residential_rent(&candidates))
    }
}

/// Lazy rent lookup receives the current business records after preceding moves.
pub type AverageRent<'a> = dyn FnMut(&[[u8; 544]]) -> Result<f32, String> + 'a;

///00463f70 applies command0x18's operational fields. A rent increase (including
/// unordered x87 comparison) reevaluates the linked tenants immediately. Capture
/// each next link before reevaluation can move that tenant to another list.
/// This operation does not invalidate the original market cache.
pub fn apply_business_terms(
    residents: &mut ResidentAccounts,
    world: &mut World<'_>,
    business_index: usize,
    terms: &crate::economy::BusinessTerms,
    objectives: &DailyObjectives,
    objective_rng: &mut u32,
) -> Result<(), String> {
    let business = world
        .businesses
        .get_mut(business_index)
        .ok_or("business settings target leaves pool")?;
    for (i, price) in terms.prices.iter().enumerate() {
        business[0x1c4 + i * 4..0x1c8 + i * 4].copy_from_slice(&price.to_le_bytes());
    }
    let old_rent = f32::from_le_bytes(business[0x1dc..0x1e0].try_into().unwrap());
    for (i, target) in terms.stock_targets.iter().enumerate() {
        business[0x1b8 + i * 2..0x1ba + i * 2].copy_from_slice(&target.to_le_bytes());
    }
    for (i, price) in terms.prices.iter().enumerate() {
        business[0x1ec + i * 4..0x1f0 + i * 4].copy_from_slice(&price.to_le_bytes());
    }
    for (i, target) in terms.stock_targets.iter().enumerate() {
        business[0x1e0 + i * 2..0x1e2 + i * 2].copy_from_slice(&target.to_le_bytes());
    }
    business[0x1dc..0x1e0].copy_from_slice(&terms.rent.to_le_bytes());
    if old_rent < terms.rent || old_rent.is_nan() || terms.rent.is_nan() {
        let mut next = i32::from_le_bytes(business[0x20c..0x210].try_into().unwrap());
        let mut seen = vec![false; residents.nodes.len()];
        while next != -1 {
            let index = usize::try_from(next).map_err(|_| "negative business tenant index")?;
            let node = residents
                .nodes
                .get(index)
                .ok_or("business tenant leaves pool")?;
            if seen[index] {
                return Err("cyclic business tenant chain".into());
            }
            seen[index] = true;
            next = node.next_in_housing();
            reevaluate(residents, index, world, objectives, objective_rng)?;
        }
    }
    world.businesses[business_index][0x204..0x208].copy_from_slice(&terms.rent.to_le_bytes());
    Ok(())
}

/// Borrowed simulation state, not a second copy of resident/business records.
pub struct World<'a> {
    pub demand: Option<&'a mut crate::ai_demand::State>,
    pub buildings: &'a [Building],
    pub categories: &'a [Category; 52],
    /// Current shared category prestige; runtime housing must not read a stale
    /// value captured when its geometry/topology was loaded.
    pub category_values: &'a [f32; 52],
    pub businesses: &'a mut [[u8; 0x220]],
    pub available: &'a mut Vec<usize>,
    pub players: &'a mut [PlayerAccount],
    pub welfare: &'a Welfare,
    pub settings: &'a Settings,
    ///0047f970 is called only when a category has qualifying capacity.
    pub average_rent: &'a mut AverageRent<'a>,
}

impl World<'_> {
    fn candidates(&self, category: usize) -> Result<(Vec<HousingCandidate>, Vec<usize>), String> {
        let mut candidates = Vec::new();
        let mut slots = Vec::new();
        for &slot in &self.categories[category].buildings {
            let building = self
                .buildings
                .get(slot)
                .ok_or("housing building leaves pool")?;
            let Some(index) = building.business else {
                continue;
            };
            let record = self
                .businesses
                .get(index)
                .ok_or("housing business leaves pool")?;
            candidates.push(HousingCandidate {
                residential: u32::from_le_bytes(record[..4].try_into().unwrap()) == 1,
                capacity: u16::from_le_bytes(record[0xe..0x10].try_into().unwrap()),
                occupied: u16::from_le_bytes(record[0x10..0x12].try_into().unwrap()),
                attribute_2c: building.attribute_2c,
                rent: f32::from_le_bytes(record[0x1dc..0x1e0].try_into().unwrap()),
            });
            slots.push(slot);
        }
        Ok((candidates, slots))
    }
}

/// Original45a470 chooses a preferred housing category, including residents
/// with no existing home. Strict comparisons preserve the earliest tied score;
/// failure leaves the prior preference byte untouched. This does not move them.
pub fn choose_category(
    residents: &mut ResidentAccounts,
    resident_index: usize,
    world: &mut World<'_>,
) -> Result<Option<usize>, String> {
    let resident = residents
        .nodes
        .get(resident_index)
        .ok_or("resident leaves pool")?;
    let current = match resident.housing_building() {
        -1 => None,
        value => Some(usize::try_from(value).map_err(|_| "negative housing building")?),
    };
    let current_category = resident.housing_category();
    let class = resident.class() as usize;
    let age = resident.age() as usize;
    let weights = *world
        .settings
        .weights
        .get(class)
        .ok_or("invalid housing class")?;
    let welfare = *world
        .welfare
        .amounts
        .get(class)
        .and_then(|row| row.get(age))
        .ok_or("invalid housing age/class")?;
    let mut best_score = -99_999_999;
    let mut best_category = None;
    for category in 0..52 {
        let (candidates, slots) = world.candidates(category)?;
        let same_category = i32::from(current_category) == category as i32;
        let current_index = same_category
            .then(|| current.and_then(|current| slots.iter().position(|&slot| slot == current)))
            .flatten();
        let score = score_housing_category_cached(
            &candidates,
            current_index,
            HousingScoreInputs {
                affordability_factor: world.settings.affordability_factor,
                welfare,
                rent_weight: weights[0],
                district_weight: weights[1],
                district_value: world.category_values[category],
                average_rent: 0., // supplied by the lazy getter below
                current_category: same_category,
            },
            || (world.average_rent)(world.businesses),
        )?;
        if score > best_score {
            best_score = score;
            best_category = Some(category);
        }
    }
    if let Some(category) = best_category {
        residents.nodes[resident_index].prefer_housing_category(category as u8);
    }
    Ok(best_category)
}

/// Outer00460090 action0 around the live housing helper. The pool remains
/// authoritative while assignment updates another resident's linked-list tail.
pub fn advance_dispatched_action(
    residents: &mut ResidentAccounts,
    resident_index: usize,
    auxiliary: &mut [u8; 284],
    world: &mut World<'_>,
    objectives: &DailyObjectives,
    objective_rng: &mut u32,
    mut travel: impl FnMut(
        crate::resident::HousingAction,
        &mut crate::resident::Resident,
        &mut [u8; 284],
    ) -> Result<bool, String>,
) -> Result<(), String> {
    let resident = residents
        .nodes
        .get(resident_index)
        .ok_or("resident leaves pool")?;
    if u32::from_le_bytes(resident.as_bytes()[0xcc..0xd0].try_into().unwrap()) != 0 {
        return Err("housing dispatch requires action0".into());
    }
    let done = advance_action(
        residents,
        resident_index,
        world,
        objectives,
        objective_rng,
        |helper, resident| travel(helper, resident, auxiliary),
    )?;
    residents.nodes[resident_index].finish_action_transition(done.then_some((1, 1)), auxiliary);
    Ok(())
}

/// Original0045fa40 using live housing selection and assignment. Travel
/// helpers remain explicit host-supplied simulation operations; they must not
/// report completion until their original movement conditions are satisfied.
pub fn advance_action(
    residents: &mut ResidentAccounts,
    resident_index: usize,
    world: &mut World<'_>,
    objectives: &DailyObjectives,
    objective_rng: &mut u32,
    mut travel: impl FnMut(
        crate::resident::HousingAction,
        &mut crate::resident::Resident,
    ) -> Result<bool, String>,
) -> Result<bool, String> {
    use crate::resident::HousingAction;
    let resident = residents
        .nodes
        .get(resident_index)
        .ok_or("resident leaves pool")?;
    let Some(action) = resident.housing_action_step() else {
        return Ok(false);
    };
    let success = match action {
        HousingAction::Choose => choose_category(residents, resident_index, world)?.is_some(),
        HousingAction::Assign => {
            assign_preferred(residents, resident_index, world, objectives, objective_rng)?
        }
        HousingAction::TravelA | HousingAction::TravelB => {
            travel(action, &mut residents.nodes[resident_index])?
        }
    };
    Ok(residents.nodes[resident_index].finish_housing_action(action, success))
}

/// Assignment branch45fab9..45fbcc, after action state7 has been reached.
/// Returns false when no residence qualifies. The outer action state machine
/// owns its fallback transition; this helper does not bypass travel/selection.
pub fn assign_preferred(
    residents: &mut ResidentAccounts,
    resident_index: usize,
    world: &mut World<'_>,
    objectives: &DailyObjectives,
    objective_rng: &mut u32,
) -> Result<bool, String> {
    let resident = residents
        .nodes
        .get(resident_index)
        .ok_or("resident leaves pool")?;
    let category = usize::from(resident.as_bytes()[0x7d]);
    if category >= 52 || resident.age() > 4 {
        return Err("invalid resident housing selector".into());
    }
    let weights = world
        .settings
        .weights
        .get(resident.class() as usize)
        .ok_or("invalid housing class")?;
    let welfare = world.welfare.amounts[resident.class() as usize][resident.age() as usize];
    let current = resident.housing_building();
    let (candidates, slots) = world.candidates(category)?;
    let current_index = slots
        .iter()
        .position(|&slot| current >= 0 && slot == current as usize);
    let Some(index) = select_residence(
        &candidates,
        current_index,
        world.settings.affordability_factor,
        welfare,
        weights[0],
        weights[2],
    )?
    else {
        return Ok(false);
    };
    let business = world.buildings[slots[index]]
        .business
        .ok_or("selected residence has no business")?;
    let target = i16::from_le_bytes(world.businesses[business][8..10].try_into().unwrap());
    let building = world
        .buildings
        .get(usize::try_from(target).map_err(|_| "negative housing target")?)
        .ok_or("housing target leaves pool")?;
    // The original caller ignores append's boolean and proceeds with the address.
    let appended = residents.append_housing_resident(
        &mut world.businesses[business],
        business,
        resident_index,
        world.available,
    )?;
    if appended
        && u32::from_le_bytes(world.businesses[business][..4].try_into().unwrap()) == 1
        && let Some(demand) = world.demand.as_deref_mut()
    {
        demand.resident_changed(&residents.nodes[resident_index], 1, 1)?;
    }
    residents.nodes[resident_index].relocate(
        target,
        building.category,
        Some(building.housing_position),
    );
    residents.nodes[resident_index].refresh_daily_objective(objectives, objective_rng);
    let owner = world.businesses[business][0xa] as i8;
    if owner < 0 {
        return Err("invalid signed housing owner".into());
    }
    if let Some(player) = owner
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| world.players.get_mut(index))
    {
        player.housing_counter = player.housing_counter.wrapping_add(50);
    }
    Ok(true)
}

/// Complete original 0045a770 for valid resident/class data. UI notification
/// delivery remains a host concern; address, objective RNG and accounting are
/// updated here. The hour17 countdown is the caller's separate responsibility.
pub fn reevaluate(
    residents: &mut ResidentAccounts,
    resident_index: usize,
    world: &mut World<'_>,
    objectives: &DailyObjectives,
    rng: &mut u32,
) -> Result<(), String> {
    let resident = residents
        .nodes
        .get(resident_index)
        .ok_or("resident leaves pool")?;
    let current = resident.housing_building();
    if current == -1 {
        return Ok(());
    }
    let current = usize::try_from(current).map_err(|_| "negative housing building")?;
    let current_category = resident.housing_category();
    let class = resident.class() as usize;
    let age = resident.age() as usize;
    let weights = world
        .settings
        .weights
        .get(class)
        .ok_or("invalid housing class")?;
    let welfare = *world
        .welfare
        .amounts
        .get(class)
        .and_then(|row| row.get(age))
        .ok_or("invalid housing age/class")?;
    let weights = *weights;
    let current_business = world
        .buildings
        .get(current)
        .and_then(|b| b.business)
        .ok_or("current housing has no business")?;
    let best_category = choose_category(residents, resident_index, world)?;
    let mut destination = None;
    if let Some(category) = best_category {
        let (candidates, slots) = world.candidates(category)?;
        let same_category = i32::from(current_category) == category as i32;
        let current_index = same_category
            .then(|| slots.iter().position(|&slot| slot == current))
            .flatten();
        if let Some(index) = select_residence(
            &candidates,
            current_index,
            world.settings.affordability_factor,
            welfare,
            weights[0],
            weights[2],
        )? {
            let business = world.buildings[slots[index]].business.unwrap();
            let target = i16::from_le_bytes(world.businesses[business][8..10].try_into().unwrap());
            if target == current as i16 {
                return Ok(());
            }
            let building = world
                .buildings
                .get(usize::try_from(target).map_err(|_| "negative destination")?)
                .ok_or("destination leaves building pool")?;
            destination = Some((
                business,
                target,
                building.category,
                building.housing_position,
            ));
        }
    }
    let record = world
        .businesses
        .get_mut(current_business)
        .ok_or("current business leaves pool")?;
    let owner = record[0xa] as i8;
    if owner < 0 {
        return Err("invalid signed housing owner".into());
    }
    let owner = owner
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| world.players.get_mut(index));
    let removed = residents.remove_housing_resident(
        record,
        current_business,
        resident_index,
        world.available,
        owner,
    )?;
    if removed
        && u32::from_le_bytes(record[..4].try_into().unwrap()) == 1
        && let Some(demand) = world.demand.as_deref_mut()
    {
        demand.resident_changed(&residents.nodes[resident_index], 1, 0)?;
    }
    if let Some((business, target, category, position)) = destination {
        // Original caller ignores append's boolean, including overcapacity.
        let appended = residents.append_housing_resident(
            &mut world.businesses[business],
            business,
            resident_index,
            world.available,
        )?;
        if appended
            && u32::from_le_bytes(world.businesses[business][..4].try_into().unwrap()) == 1
            && let Some(demand) = world.demand.as_deref_mut()
        {
            demand.resident_changed(&residents.nodes[resident_index], 1, 1)?;
        }
        residents.nodes[resident_index].relocate(target, category, Some(position));
    } else {
        residents.nodes[resident_index].relocate(-1, -1, None);
    }
    residents.nodes[resident_index].refresh_daily_objective(objectives, rng);
    Ok(())
}

/// Original housing countdown RNG at 6fcd40/44/48 and period at 6fcd38.
/// Initialization/loading provenance is separate from the observed update rule.
pub struct Countdown {
    pub state: u32,
    pub multiplier: u32,
    pub increment: u32,
    pub period: i32,
}

/// Complete 0045bdd0: decrement signed four-bit deadlines in resident pool
/// order, draw the next deadline, then reevaluate housing. Objective RNG is a
/// separate stream consumed only by moves/evictions, not unchanged residences.
pub fn update_evening(
    residents: &mut ResidentAccounts,
    world: &mut World<'_>,
    objectives: &DailyObjectives,
    objective_rng: &mut u32,
    countdown: &mut Countdown,
) -> Result<(), String> {
    if countdown.period == 0 {
        return Err("zero housing countdown period".into());
    }
    for index in 0..residents.iteration_len()? {
        if residents.nodes[index].tick_housing_countdown() {
            countdown.state = countdown
                .state
                .wrapping_mul(countdown.multiplier)
                .wrapping_add(countdown.increment);
            let next = ((countdown.state >> 16) & 0x7fff) as i32 % countdown.period + 1;
            residents.nodes[index].set_housing_countdown(next);
            reevaluate(residents, index, world, objectives, objective_rng)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn housing_average_adapter_retains_capacity_weighted_rent() {
        let topology = Topology {
            buildings: (0..2)
                .map(|index| Building {
                    business: Some(index),
                    category: 0,
                    attribute_2c: 0,
                    housing_position: [0.; 3],
                })
                .collect(),
            categories: std::array::from_fn(|_| Category {
                buildings: vec![],
                value: 0.,
                center: None,
                resident_counter: 0,
            }),
            active: vec![true; 2],
        };
        let mut businesses = [[0u8; 0x220]; 2];
        for (row, (capacity, rent)) in businesses
            .iter_mut()
            .zip([(1u16, 2f32.powi(-42)), (4097, 1. + 2f32.powi(-12))])
        {
            row[..4].copy_from_slice(&1u32.to_le_bytes());
            row[0xe..0x10].copy_from_slice(&capacity.to_le_bytes());
            row[0x1dc..0x1e0].copy_from_slice(&rent.to_le_bytes());
        }
        assert_eq!(
            topology.average_rent(&businesses).unwrap().to_bits(),
            0x3f80_0001
        );
    }

    #[test]
    #[ignore = "requires original two-tick housing fixture and timetable"]
    fn matches_native_housing_ticks() {
        use crate::resident::{Resident, TickEvent, Timetables};
        let data = std::fs::read(crate::fixtures::path("NATIVE_HOUSING_TICK")).unwrap();
        assert_eq!(&data[..8], b"MTHATK01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!((word(12), word(16), word(20)), (4, 4, 0x037f));
        let objectives = DailyObjectives::from_timetable(
            &std::fs::read_to_string(crate::fixtures::path("TIMETABLE")).unwrap(),
        )
        .unwrap();
        let timetable = Timetables::from_timetable("TIMETABLE 0 HOUR9 LOOK_FOR_A_HOUSE").unwrap();
        let mut at = 24;
        for case in 0..word(8) {
            at += 4; // labelled affordability, determined from real terms below
            let mut rng = [word(at), word(at + 4), word(at + 8)];
            let expected_rng = [word(at + 12), word(at + 16), word(at + 20)];
            at += 24;
            let mut objective_rng = word(at);
            assert_eq!((word(at + 4), word(at + 8)), (0x343fd, 0x269ec3));
            let expected_objective_rng = word(at + 12);
            at += 24;
            let centers = std::array::from_fn(|i| {
                [
                    f32::from_bits(word(at + i * 8)),
                    f32::from_bits(word(at + i * 8 + 4)),
                ]
            });
            at += 416;
            let count = word(at) as usize;
            at += 4;
            let expected_available: Vec<_> =
                (0..count).map(|i| word(at + i * 4) as usize).collect();
            at += count * 4;
            let count = word(at) as usize;
            at += 4;
            let expected_calls: Vec<_> = (0..count)
                .filter_map(|i| {
                    let address = word(at + i * 12);
                    matches!(
                        address,
                        0x460470 | 0x45fc90 | 0x460090 | 0x45fa40 | 0x45f1c0
                    )
                    .then_some((
                        address,
                        word(at + i * 12 + 4),
                        word(at + i * 12 + 8),
                    ))
                })
                .collect();
            at += count * 12;
            let nodes = data[at..at + 4 * 0x150]
                .chunks_exact(0x150)
                .map(|r| Resident::from_bytes(r[..0x14a].try_into().unwrap()))
                .collect();
            at += 4 * 0x150;
            let expected_nodes = [
                &data[at..at + 4 * 0x150],
                &data[at + 4 * 0x150..at + 8 * 0x150],
            ];
            at += 8 * 0x150;
            let mut auxiliaries: Vec<[u8; 284]> = data[at..at + 4 * 284]
                .chunks_exact(284)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 4 * 284;
            let expected_aux = [&data[at..at + 4 * 284], &data[at + 4 * 284..at + 8 * 284]];
            at += 8 * 284;
            let mut businesses: Vec<[u8; 544]> = data[at..at + 4 * 544]
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 4 * 544;
            let expected_businesses = &data[at..at + 4 * 544];
            at += 4 * 544;
            let mut residents = ResidentAccounts {
                active_count: 4,
                nodes,
                accounts: vec![],
            };
            let buildings: Vec<_> = (0..4)
                .map(|i| Building {
                    business: Some(i),
                    category: 0,
                    attribute_2c: 0,
                    housing_position: [0.; 3],
                })
                .collect();
            let categories = std::array::from_fn(|i| Category {
                buildings: if i == 0 { vec![0] } else { vec![] },
                value: 0.,
                center: Some(centers[i]),
                resident_counter: 0,
            });
            let mut available = vec![0, 1, 2, 3];
            let mut average = |businesses: &[[u8; 544]]| {
                Ok(f32::from_le_bytes(
                    businesses[0][0x1dc..0x1e0].try_into().unwrap(),
                ))
            };
            let mut world = World {
                demand: None,
                buildings: &buildings,
                categories: &categories,
                category_values: &[0.; 52],
                businesses: &mut businesses,
                available: &mut available,
                players: &mut [],
                welfare: &Welfare {
                    amounts: [[120.; 5]; 3],
                },
                settings: &Settings {
                    affordability_factor: 0.48,
                    weights: [[1.; 3]; 3],
                },
                average_rent: &mut average,
            };
            let mut calls = Vec::new();
            for tick in 0..2 {
                residents
                    .advance_population(|index, pool| {
                        calls.push((0x460470, index as u32, tick as u32));
                        pool.tick_resident(
                            index,
                            &mut auxiliaries[index],
                            &centers,
                            |event, index, pool, auxiliary| {
                                let address = match event {
                                    TickEvent::Movement => panic!("unexpected movement"),
                                    TickEvent::RefreshIntent => 0x45fc90,
                                    TickEvent::Action => 0x460090,
                                };
                                calls.push((address, index as u32, tick as u32));
                                match event {
                                    TickEvent::RefreshIntent => {
                                        pool.nodes[index].refresh_intent(&timetable, 9, [0; 5])?
                                    }
                                    TickEvent::Action => {
                                        let action = u32::from_le_bytes(
                                            pool.nodes[index].as_bytes()[0xcc..0xd0]
                                                .try_into()
                                                .unwrap(),
                                        );
                                        if action == 0 {
                                            calls.push((0x45fa40, index as u32, tick as u32));
                                            advance_dispatched_action(
                                                pool,
                                                index,
                                                auxiliary,
                                                &mut world,
                                                &objectives,
                                                &mut objective_rng,
                                                |_, _, _| panic!("unexpected travel"),
                                            )?;
                                        } else {
                                            assert_eq!(action, 12);
                                            pool.nodes[index].advance_action(
                                                auxiliary,
                                                |helper, resident, auxiliary| {
                                                    assert_eq!(
                                                        helper,
                                                        crate::resident::ActionHelper::At45f1c0
                                                    );
                                                    calls.push((
                                                        0x45f1c0,
                                                        index as u32,
                                                        tick as u32,
                                                    ));
                                                    resident.advance_roaming_action(
                                                        auxiliary,
                                                        &[1; 52],
                                                        &mut rng,
                                                        |_, _, _| {
                                                            panic!("unexpected roaming travel")
                                                        },
                                                    )
                                                },
                                            )?;
                                        }
                                    }
                                    TickEvent::Movement => unreachable!(),
                                }
                                Ok(false)
                            },
                        )
                    })
                    .unwrap();
                for (i, resident) in residents.nodes.iter().enumerate() {
                    assert_eq!(
                        resident.as_bytes().as_slice(),
                        &expected_nodes[tick][i * 0x150..i * 0x150 + 0x14a],
                        "case{case} tick{tick} node{i}"
                    );
                    assert_eq!(
                        auxiliaries[i].as_slice(),
                        &expected_aux[tick][i * 284..(i + 1) * 284],
                        "case{case} tick{tick} aux{i}"
                    );
                }
            }
            assert_eq!(rng, expected_rng);
            assert_eq!(objective_rng, expected_objective_rng);
            assert_eq!(calls, expected_calls);
            assert_eq!(available, expected_available);
            for (i, business) in businesses.iter().enumerate() {
                assert_eq!(
                    business.as_slice(),
                    &expected_businesses[i * 544..(i + 1) * 544]
                );
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original saves, assets, settings and native saved evening fixture"]
    fn matches_native_saved_housing_evening() {
        check_saved_cycles(false, false);
    }

    #[test]
    #[ignore = "requires original hourly resident-cycle observations"]
    fn matches_native_hourly_resident_cycles() {
        check_saved_cycles(true, false);
    }

    #[test]
    #[ignore = "requires original hourly resident cycles with dirty market cache"]
    fn matches_native_hourly_market_cycles() {
        check_saved_cycles(true, true);
    }

    fn check_saved_cycles(hourly: bool, market_dirty: bool) {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let build_data = read("OPENMT2001_BUILD_FILE");
        let build_file = crate::build_file::BuildFile::parse(&build_data).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let settings =
            Settings::from_parameters(std::str::from_utf8(&parameters).unwrap(), &image).unwrap();
        let defaults = read("OPENMT2001_DEFAULT_SETTINGS");
        let welfare = Welfare::from_v14_defaults(std::str::from_utf8(&defaults).unwrap()).unwrap();
        let timetable = read("OPENMT2001_TIMETABLE");
        let objectives =
            DailyObjectives::from_timetable(std::str::from_utf8(&timetable).unwrap()).unwrap();
        let saves = [read("OPENMT2001_SAVE_ONE"), read("OPENMT2001_SAVE_TWO")];
        let bytes = read(if market_dirty {
            "OPENMT2001_NATIVE_HOURLY_MARKET"
        } else if hourly {
            "OPENMT2001_NATIVE_HOURLY_RESIDENTS"
        } else {
            "OPENMT2001_NATIVE_SAVED_HOUSING_EVENING"
        });
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(
            &bytes[..8],
            if market_dirty {
                b"MTHRMK01"
            } else if hourly {
                b"MTHRSEN1"
            } else {
                b"MTHSVEN1"
            }
        );
        assert_eq!((word(8), word(12), word(16)), (4, 3, 0x027f));
        let mut at = 20;
        for case in 0..4 {
            at += 32; // Original input SHA-256, retained for reproduction.
            let save_index = word(at) as usize;
            assert_eq!(save_index, case / 2);
            let save = crate::save::SaveFile::parse(&saves[save_index]).unwrap();
            let node_count = word(at + 4) as usize;
            let player_count = word(at + 8) as usize;
            let before_count = word(at + 12) as usize;
            let after_count = word(at + 16) as usize;
            let expected_countdown = word(at + 20);
            let expected_objective = word(at + 24);
            let mut countdown = Countdown {
                state: 0x11223344,
                multiplier: 0x343fd,
                increment: 0x269ec3,
                period: word(at + 28) as i32,
            };
            at += 32;
            let mut available: Vec<_> = (0..before_count)
                .map(|i| word(at + i * 4) as usize)
                .collect();
            at += before_count * 4;
            let expected_available: Vec<_> = (0..after_count)
                .map(|i| word(at + i * 4) as usize)
                .collect();
            at += after_count * 4;
            let topology = Topology::reconstruct(&save, &board, &build_file).unwrap();
            let crate::economy::Ledgers {
                mut businesses,
                mut residents,
                mut players,
            } = crate::economy::Ledgers::from_saved(&save).unwrap();
            assert_eq!(residents.nodes.len(), node_count);
            assert_eq!(players.len(), player_count);
            let original_accounts: Vec<_> =
                residents.accounts.iter().map(|a| *a.as_bytes()).collect();
            let average_rent = topology.average_rent(&businesses).unwrap();
            let mut objective_rng = 0x12345678;
            let mut market = crate::market::Cache {
                dirty: market_dirty,
                prices: if market_dirty { [-123.; 32] } else { [0.; 32] },
                rent: if market_dirty { -456. } else { average_rent },
            };
            if hourly {
                let business_defaults = read("OPENMT2001_BUSINESS_SETTINGS");
                let shopping_settings = crate::shopping::Settings::from_saved(
                    &save,
                    &image,
                    std::str::from_utf8(&defaults).unwrap(),
                    std::str::from_utf8(&business_defaults).unwrap(),
                    std::str::from_utf8(&parameters).unwrap(),
                )
                .unwrap();
                let mut shopping_topology =
                    crate::shopping::Topology::reconstruct(&save, &board, &build_file).unwrap();
                let business_settings = save.economy().unwrap().daily_settings();
                let mut prefixes: Vec<[u8; 64]> = save
                    .building_prefixes()
                    .unwrap()
                    .chunks_exact(64)
                    .map(|p| p.try_into().unwrap())
                    .collect();
                let mut utilities = crate::economy::Utilities::new(save.utility_owners().unwrap());
                let mut properties = crate::economy::PropertyIncome {
                    owners: [0; 4],
                    totals: [0.; 4],
                };
                let mut finance = crate::hourly::Finance {
                    topology: &mut shopping_topology,
                    buildings: &mut prefixes,
                    shopping: &shopping_settings,
                    business: &business_settings,
                    utilities: &mut utilities,
                    properties: &mut properties,
                    income: crate::economy::OwnerIncomeSettings::original_hub(
                        save.difficulty().unwrap(),
                    )
                    .unwrap(),
                };
                let mut ledgers = crate::economy::Ledgers {
                    businesses,
                    residents,
                    players,
                };
                let mut resident_hours = crate::hourly::Residents {
                    housing: &topology,
                    housing_settings: &settings,
                    welfare: &welfare,
                    objectives: &objectives,
                    objective_rng: &mut objective_rng,
                    housing_countdown: &mut countdown,
                    available: &mut available,
                    market: &mut market,
                };
                let mut clock = save.clock().unwrap();
                let mut controls = crate::hourly::Controls {
                    suppressed: false,
                    world_mode: 3,
                    before_six_sound: 0,
                    network_enabled: true,
                    network_override: false,
                };
                for _ in 0..3 {
                    for hour in [5, 17] {
                        clock.calendar[0] = hour;
                        resident_hours
                            .advance(
                                &mut finance,
                                &mut clock,
                                &mut controls,
                                &mut ledgers,
                                |_, _, _, _, _, _| Ok(false),
                            )
                            .unwrap();
                    }
                }
                crate::economy::Ledgers {
                    businesses,
                    residents,
                    players,
                } = ledgers;
            } else {
                let mut cached_average = |_: &[[u8; 544]]| Ok(average_rent);
                let mut world = World {
                    demand: None,
                    buildings: &topology.buildings,
                    categories: &topology.categories,
                    category_values: &std::array::from_fn(|i| topology.categories[i].value),
                    businesses: &mut businesses,
                    available: &mut available,
                    players: &mut players,
                    welfare: &welfare,
                    settings: &settings,
                    average_rent: &mut cached_average,
                };
                for _ in 0..3 {
                    update_evening(
                        &mut residents,
                        &mut world,
                        &objectives,
                        &mut objective_rng,
                        &mut countdown,
                    )
                    .unwrap();
                }
            }
            assert_eq!(
                countdown.state, expected_countdown,
                "housing RNG case {case}"
            );
            assert_eq!(
                objective_rng, expected_objective,
                "objective RNG case {case}"
            );
            assert_eq!(available, expected_available, "availability case {case}");
            for (slot, business) in businesses.iter().enumerate() {
                assert_eq!(
                    &business[..],
                    &bytes[at..at + 0x220],
                    "business {slot} case {case}"
                );
                at += 0x220;
            }
            for (slot, node) in residents.nodes.iter().enumerate() {
                assert_eq!(
                    &node.as_bytes()[..],
                    &bytes[at..at + 0x14a],
                    "resident {slot} case {case}"
                );
                at += 0x14a;
            }
            if hourly {
                for (index, account) in residents.accounts.iter().enumerate() {
                    assert_eq!(
                        account.as_bytes().as_slice(),
                        &bytes[at..at + 144],
                        "case{case} spending account{index}"
                    );
                    at += 144;
                }
            }
            for player in players {
                assert_eq!(player.housing_counter, word(at) as i32, "owner case {case}");
                at += 4;
            }
            if market_dirty {
                assert_eq!(
                    u32::from(market.dirty),
                    word(at),
                    "case {case} market dirty"
                );
                for (i, price) in market.prices.iter().enumerate() {
                    assert_eq!(
                        price.to_bits(),
                        word(at + 4 + i * 4),
                        "case {case} market price {i}"
                    );
                }
                assert_eq!(
                    market.rent.to_bits(),
                    word(at + 132),
                    "case {case} market rent"
                );
                at += 136;
            }
            for (account, original) in residents
                .accounts
                .iter()
                .zip(original_accounts)
                .filter(|_| !hourly)
            {
                assert_eq!(
                    account.as_bytes(),
                    &original,
                    "spending account case {case}"
                );
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires both original saves and native saved housing topology observations"]
    fn matches_native_saved_housing() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let board =
            crate::board::Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let build_data = std::fs::read(crate::fixtures::path("BUILD_FILE")).unwrap();
        let file = crate::build_file::BuildFile::parse(&build_data).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_SAVED_HOUSING")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTHSAVE1");
        assert_eq!(word(8), 2);
        let mut at = 12;
        for (name, expected_housed) in [("OPENMT2001_SAVE_ONE", 264), ("OPENMT2001_SAVE_TWO", 0)] {
            let data = std::fs::read(crate::fixtures::path(name)).unwrap();
            let save = crate::save::SaveFile::parse(&data).unwrap();
            // This fixture isolates topology/cache from the geometry rebuild.
            // Coordinates supplied here are explicit probe inputs, not recovered
            // empty-building positions or an asserted saved geometry result.
            let positions = std::array::from_fn(|i| [i as f32, 6., -(i as f32)]);
            let topology = Topology::from_saved(&save, &positions).unwrap();
            let reconstructed = Topology::reconstruct(&save, &board, &file).unwrap();
            let mut occupied = 0;
            for node in save.residents().unwrap().nodes {
                let building = i16::from_le_bytes(node[0xe..0x10].try_into().unwrap());
                if building < 0 {
                    continue;
                }
                occupied += 1;
                let position = reconstructed.buildings[building as usize].housing_position;
                for (axis, offset) in [(0, 0x10), (2, 0x18)] {
                    assert_eq!(
                        position[axis].to_bits(),
                        u32::from_le_bytes(node[offset..offset + 4].try_into().unwrap()),
                        "saved resident home {name} building {building} axis {axis}"
                    );
                }
            }
            assert_eq!(occupied, expected_housed, "saved housed population {name}");
            at += 32; // Source digest is retained in the fixture for reproduction.
            for category in &topology.categories {
                assert_eq!(category.value.to_bits(), word(at));
                let count = word(at + 4) as usize;
                at += 8;
                assert_eq!(category.buildings.len(), count);
                for &slot in &category.buildings {
                    assert_eq!(slot, word(at) as usize);
                    assert_eq!(topology.buildings[slot].housing_position, positions[slot]);
                    at += 4;
                }
            }
            let businesses: Vec<_> = save
                .economy()
                .unwrap()
                .businesses()
                .map(|b| *b.bytes)
                .collect();
            let average = topology.average_rent(&businesses).unwrap();
            let expected = f32::from_bits(word(at));
            if expected.is_nan() {
                assert!(average.is_nan());
            } else {
                assert_eq!(average.to_bits(), expected.to_bits());
            }
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original class-setting callback fixture and executable"]
    fn matches_native_housing_settings() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_HOUSING_SETTINGS")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTHSET01");
        assert_eq!((word(8), word(12)), (6, 0x027f));
        let mut at = 16;
        for case in 0..word(8) {
            let length = word(at) as usize;
            let calls = word(at + 4);
            at += 8;
            let source = std::str::from_utf8(&bytes[at..at + length]).unwrap();
            assert_eq!(
                source
                    .lines()
                    .filter(|line| line
                        .split_whitespace()
                        .next()
                        .is_some_and(|s| s.eq_ignore_ascii_case("ClassWeighting")))
                    .count(),
                calls as usize
            );
            let settings = Settings::from_parameters(source, &image).unwrap();
            at += length;
            assert_eq!(settings.affordability_factor.to_bits(), word(at));
            at += 4;
            for class in settings.weights {
                for weight in class {
                    assert_eq!(weight.to_bits(), word(at), "class setting case {case}");
                    at += 4;
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original complete housing reevaluation fixture and timetable"]
    fn matches_native_housing_move() {
        check_native(false, false, false);
    }
    #[test]
    #[ignore = "requires original full evening housing fixture and timetable"]
    fn matches_native_housing_evening() {
        check_native(true, false, false);
    }

    #[test]
    #[ignore = "requires original business terms command fixture and timetable"]
    fn matches_native_business_terms() {
        check_native(false, true, false);
    }

    #[test]
    #[ignore = "requires original housing assignment fixture and timetable"]
    fn matches_native_housing_assignment() {
        check_native(false, false, true);
    }

    fn check_native(evening: bool, terms: bool, assignment: bool) {
        let bytes = std::fs::read(crate::fixtures::path(if assignment {
            "OPENMT2001_NATIVE_HOUSING_ASSIGNMENT"
        } else if terms {
            "OPENMT2001_NATIVE_BUSINESS_TERMS"
        } else if evening {
            "OPENMT2001_NATIVE_HOUSING_EVENING"
        } else {
            "OPENMT2001_NATIVE_HOUSING_MOVE"
        }))
        .unwrap();
        let timetable = std::fs::read_to_string(crate::fixtures::path("TIMETABLE")).unwrap();
        let objectives = DailyObjectives::from_timetable(&timetable).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(
            &bytes[..8],
            if assignment {
                b"MTHASG01"
            } else if terms {
                b"MTHTERM1"
            } else if evening {
                b"MTHEVEN1"
            } else {
                b"MTHMOVE1"
            }
        );
        assert_eq!((word(8), word(12), word(16)), (25, 4, 4));
        let mut at = 20;
        if assignment {
            assert_eq!(word(at), 0x037f);
            at += 4;
        }
        for case in 0..word(8) {
            let command = terms.then(|| {
                let index = word(at) as usize;
                let terms = crate::economy::BusinessTerms {
                    stock_targets: std::array::from_fn(|i| {
                        i16::from_le_bytes(
                            bytes[at + 4 + i * 2..at + 6 + i * 2].try_into().unwrap(),
                        )
                    }),
                    prices: std::array::from_fn(|i| float(at + 16 + i * 4)),
                    rent: float(at + 40),
                };
                at += 44;
                (index, terms)
            });
            let expected_assignment = if assignment {
                let result = word(at) != 0;
                at += 4;
                Some(result)
            } else {
                None
            };
            let affordability_factor = float(at);
            let average_rent = float(at + 4);
            let weights = [float(at + 8), float(at + 12), float(at + 16)];
            let mut rng = word(at + 20);
            let expected_rng = word(at + 24);
            let counter = word(at + 28);
            let expected_counter = word(at + 32) as i32;
            at += 36;
            let mut countdown = Countdown {
                state: 0,
                multiplier: 0,
                increment: 0,
                period: 1,
            };
            let mut expected_countdown = 0;
            if evening {
                countdown = Countdown {
                    state: word(at),
                    multiplier: word(at + 8),
                    increment: word(at + 12),
                    period: word(at + 16) as i32,
                };
                expected_countdown = word(at + 4);
                at += 20;
            }
            let mut categories: [Category; 52] = std::array::from_fn(|i| Category {
                buildings: vec![],
                value: float(at + i * 4),
                center: None,
                resident_counter: 0,
            });
            at += 52 * 4;
            let mut buildings = Vec::new();
            for i in 0..4 {
                let category = word(at) as i8;
                buildings.push(Building {
                    business: Some(i),
                    category,
                    attribute_2c: word(at + 4) as i8,
                    housing_position: [float(at + 8), float(at + 12), float(at + 16)],
                });
                categories[category as usize].buildings.push(i);
                at += 20;
            }
            let before_count = word(at) as usize;
            let after_count = word(at + 4) as usize;
            at += 8;
            let mut available: Vec<_> = (0..before_count)
                .map(|i| word(at + i * 4) as usize)
                .collect();
            at += before_count * 4;
            let expected_available: Vec<_> = (0..after_count)
                .map(|i| word(at + i * 4) as usize)
                .collect();
            at += after_count * 4;
            let mut businesses: Vec<[u8; 0x220]> = bytes[at..at + 4 * 0x220]
                .chunks_exact(0x220)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 4 * 0x220;
            let expected_businesses = &bytes[at..at + 4 * 0x220];
            at += 4 * 0x220;
            let nodes = bytes[at..at + 4 * 0x14a]
                .chunks_exact(0x14a)
                .map(|r| crate::resident::Resident::from_bytes(r.try_into().unwrap()))
                .collect();
            at += 4 * 0x14a;
            let expected_nodes = &bytes[at..at + 4 * 0x14a];
            at += 4 * 0x14a;
            let mut residents = ResidentAccounts {
                active_count: 4,
                nodes,
                accounts: vec![],
            };
            let mut player_bytes = [0_u8; 0x1b8];
            player_bytes[0x1ac..0x1b0].copy_from_slice(&counter.to_le_bytes());
            let mut players = [PlayerAccount::from_saved(&crate::save::SavedPlayer {
                metadata: &[0; 0x30],
                bytes: &player_bytes,
            })
            .unwrap()];
            let mut cached_average = |_: &[[u8; 544]]| Ok(average_rent);
            let mut world = World {
                demand: None,
                buildings: &buildings,
                categories: &categories,
                category_values: &std::array::from_fn(|i| categories[i].value),
                businesses: &mut businesses,
                available: &mut available,
                players: &mut players,
                welfare: &Welfare {
                    amounts: [[50.; 5]; 3],
                },
                settings: &Settings {
                    weights: [weights; 3],
                    affordability_factor,
                },
                average_rent: &mut cached_average,
            };
            if let Some((index, terms)) = command {
                // Compose the separately native-verified local transport with
                // the actual shared-world application and tenant side effects.
                let mut commands = crate::economy::LocalBusinessCommands::default();
                commands
                    .submit(
                        crate::economy::PendingBusinessTerms {
                            business_index: index as u32,
                            terms,
                        },
                        0,
                        true,
                        |_| panic!("business settings applied at submission"),
                    )
                    .unwrap();
                commands
                    .drain(&mut [24; 2], |_| panic!("business settings applied early"))
                    .unwrap();
                let mut applied = 0;
                commands
                    .drain(&mut [25; 2], |command| {
                        applied += 1;
                        apply_business_terms(
                            &mut residents,
                            &mut world,
                            command.business_index as usize,
                            &command.terms,
                            &objectives,
                            &mut rng,
                        )
                    })
                    .unwrap();
                assert_eq!(applied, 1);
            } else if assignment {
                let assigned = advance_action(
                    &mut residents,
                    0,
                    &mut world,
                    &objectives,
                    &mut rng,
                    |_, _| panic!("unexpected travel in assignment oracle"),
                )
                .unwrap();
                assert_eq!(
                    Some(assigned),
                    expected_assignment,
                    "assignment result case {case}"
                );
            } else if evening {
                update_evening(
                    &mut residents,
                    &mut world,
                    &objectives,
                    &mut rng,
                    &mut countdown,
                )
                .unwrap();
                assert_eq!(
                    countdown.state, expected_countdown,
                    "countdown RNG case {case}"
                );
            } else {
                reevaluate(&mut residents, 0, &mut world, &objectives, &mut rng).unwrap();
            }
            assert_eq!(rng, expected_rng, "RNG case {case}");
            assert_eq!(
                players[0].housing_counter, expected_counter,
                "owner case {case}"
            );
            assert_eq!(available, expected_available, "availability case {case}");
            for (business, expected) in businesses
                .iter()
                .zip(expected_businesses.chunks_exact(0x220))
            {
                assert_eq!(&business[..], expected, "business case {case}");
            }
            for (node, expected) in residents
                .nodes
                .iter()
                .zip(expected_nodes.chunks_exact(0x14a))
            {
                assert_eq!(&node.as_bytes()[..], expected, "resident case {case}");
            }
        }
        assert_eq!(at, bytes.len());
    }
}
