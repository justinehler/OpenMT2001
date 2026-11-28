//! Resident spending accounts and preference allocation, separate from scheduling.

use std::sync::{Arc, Weak};

/// Nonserialized identity. Only the real pool owns strong references. No
/// native address or caller-supplied numeric token establishes this origin.
#[derive(Debug)]
struct AuxiliaryOrigin;
#[derive(Debug)]
struct AuxiliaryAllocation;
#[derive(Clone, Debug)]
pub struct AuxiliarySource {
    origin: Weak<AuxiliaryOrigin>,
}
impl PartialEq for AuxiliarySource {
    fn eq(&self, other: &Self) -> bool {
        self.origin.ptr_eq(&other.origin)
    }
}
impl Eq for AuxiliarySource {}
impl AuxiliarySource {
    pub fn validate_live(&self) -> Result<(), String> {
        self.origin
            .upgrade()
            .map(|_| ())
            .ok_or_else(|| "resident auxiliary pool retired/replaced".into())
    }
    pub fn validate_pool(&self, pool: &AuxiliaryPool) -> Result<(), String> {
        self.validate_live()?;
        if !self.origin.ptr_eq(&Arc::downgrade(&pool.origin)) {
            return Err("resident auxiliary source belongs to another actual pool".into());
        }
        Ok(())
    }
}
/// Weak binding issued only for an allocated record by AuxiliaryPool. The
/// stored index is an access coordinate, not identity. Origin + allocation
/// weak identity survive append and reject byte-identical replacement.
#[derive(Clone, Debug)]
pub struct AuxiliaryBinding {
    source: AuxiliarySource,
    allocation: Weak<AuxiliaryAllocation>,
    index: usize,
}
impl PartialEq for AuxiliaryBinding {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.allocation.ptr_eq(&other.allocation)
    }
}
impl Eq for AuxiliaryBinding {}
impl AuxiliaryBinding {
    pub fn validate_live(&self) -> Result<(), String> {
        self.source.validate_live()?;
        self.allocation
            .upgrade()
            .map(|_| ())
            .ok_or_else(|| "resident auxiliary allocation revoked".into())
    }
    pub fn validate_source(&self, source: &AuxiliarySource) -> Result<(), String> {
        self.validate_live()?;
        source.validate_live()?;
        if self.source != *source {
            return Err("resident binding/source pool mismatch".into());
        }
        Ok(())
    }
}

/// Append-only auxiliary storage from004dfb50. Payload is unchanged; identity
/// metadata never enters slots()/save/retained byte comparisons.
pub struct AuxiliaryPool {
    slots: Vec<[u8; 0x11c]>,
    used: usize,
    origin: Arc<AuxiliaryOrigin>,
    allocations: Vec<Arc<AuxiliaryAllocation>>,
}
impl AuxiliaryPool {
    /// Original45badd..45bae7 allocates6899a0 with2000 records through4dfad0.
    pub fn for_residents() -> Self {
        Self::new(2000)
    }
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![[0; 0x11c]; capacity],
            used: 0,
            origin: Arc::new(AuxiliaryOrigin),
            allocations: Vec::new(),
        }
    }
    /// Restore full storage including unused bytes. Every occupied allocation
    /// receives a fresh identity; equal imported bytes never revive a binding.
    pub fn from_slots(slots: Vec<[u8; 0x11c]>, used: usize) -> Result<Self, String> {
        if used > slots.len() {
            return Err("auxiliary used count exceeds capacity".into());
        }
        Ok(Self {
            slots,
            used,
            origin: Arc::new(AuxiliaryOrigin),
            allocations: (0..used).map(|_| Arc::new(AuxiliaryAllocation)).collect(),
        })
    }
    /// Real reset/import writer: revoke prior allocated identities FIRST. The
    /// next pool is fully prepared before this infallible publication step.
    pub fn replace_with(&mut self, next: Self) {
        self.allocations.clear();
        *self = next; // drops old sole origin before publishing the new pool
    }
    pub fn reset_for_residents(&mut self) {
        self.replace_with(Self::for_residents());
    }
    pub fn source(&self) -> AuxiliarySource {
        AuxiliarySource {
            origin: Arc::downgrade(&self.origin),
        }
    }
    pub fn bind(&self, index: usize) -> Result<AuxiliaryBinding, String> {
        let allocation = self
            .allocations
            .get(index)
            .filter(|_| index < self.used)
            .ok_or("resident auxiliary index is unused/out of range")?;
        Ok(AuxiliaryBinding {
            source: self.source(),
            allocation: Arc::downgrade(allocation),
            index,
        })
    }
    pub fn resolve(&self, binding: &AuxiliaryBinding) -> Result<&[u8; 0x11c], String> {
        binding.source.validate_pool(self)?;
        binding.validate_live()?;
        let actual = self
            .allocations
            .get(binding.index)
            .filter(|_| binding.index < self.used)
            .ok_or("resident auxiliary binding is no longer allocated")?;
        if !binding.allocation.ptr_eq(&Arc::downgrade(actual)) {
            return Err("resident auxiliary allocation replaced".into());
        }
        self.slots
            .get(binding.index)
            .ok_or_else(|| "resident auxiliary storage absent".into())
    }
    pub fn used(&self) -> usize {
        self.used
    }
    pub fn slots(&self) -> &[[u8; 0x11c]] {
        &self.slots
    }
    pub fn slots_mut(&mut self) -> &mut [[u8; 0x11c]] {
        &mut self.slots
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut [u8; 0x11c]> {
        self.slots.get_mut(..self.used)?.get_mut(index)
    }
    /// Every byte is copied before used/identity publication. Exhaustion has
    /// no payload/identity/count effect. Appending retains prior identities.
    pub fn allocate(&mut self, template: &[u8; 0x11c]) -> Option<usize> {
        let index = self.used;
        let slot = self.slots.get_mut(index)?;
        let allocation = Arc::new(AuxiliaryAllocation);
        *slot = *template;
        self.allocations.push(allocation);
        self.used += 1;
        Some(index)
    }
}

#[cfg(test)]
#[path = "resident/auxiliary_identity_tests.rs"]
mod auxiliary_identity_tests;

/// Bind a registered model as004dfa40 does. `reference` is an opaque model
/// table reference; `definition` is the model's116-byte block at native+5c.
/// The caller resolves registered model data without dereferencing old pointers.
pub fn bind_auxiliary_model(bytes: &mut [u8; 0x11c], reference: u32, definition: &[u8; 0x74]) {
    bytes[..4].copy_from_slice(&reference.to_le_bytes());
    bytes[0x44..0xb8].copy_from_slice(definition);
}

/// Prepare the auxiliary template used by0045bb80 before004dfb50 copies it.
/// Original004df9b0/004df9c0 only initializes selected fields; callers supply
/// the prior bytes explicitly instead of reading uninitialized host memory.
pub fn initialize_auxiliary_template(bytes: &mut [u8; 0x11c], pool_index: u16) {
    initialize_auxiliary(bytes);
    //0045bb80 patches only the low word of type and the full pool-index dword.
    let kind: u16 = if pool_index & 4 == 0 { 2 } else { 1 };
    bytes[0x30..0x32].copy_from_slice(&kind.to_le_bytes());
    bytes[0x38..0x3c].copy_from_slice(&u32::from(pool_index).to_le_bytes());
}

/// Original004df9c0, shared by residents and embedded scene auxiliaries.
/// Preserve all fields not written by the original initializer.
pub fn initialize_auxiliary(bytes: &mut [u8; 0x11c]) {
    for offset in [0x00, 0x04, 0x08, 0x0c, 0x10, 0x14, 0x18, 0x34, 0x38] {
        bytes[offset..offset + 4].fill(0);
    }
    for offset in [0x1c, 0x20, 0x24] {
        bytes[offset..offset + 4].copy_from_slice(&1_f32.to_le_bytes());
    }
    for offset in [0x28, 0x3c, 0x40] {
        bytes[offset..offset + 4].fill(0xff);
    }
    bytes[0x2c..0x2e].fill(0);
    bytes[0x2e..0x30].fill(0xff);
    bytes[0x30..0x32].fill(0);
    bytes[0xb8] = (bytes[0xb8] & 0xfd) | 0x0d;
    // Original559730: three identity basis vectors followed by translation.
    for offset in [0xbc, 0xec] {
        bytes[offset..offset + 48].fill(0);
        for component in [0, 4, 8] {
            let at = offset + component * 4;
            bytes[at..at + 4].copy_from_slice(&1_f32.to_le_bytes());
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Commodity {
    pub active: bool,
    pub value: f32,
    pub markup: f32,
}

/// Shared original class settings: value, prestige, quality, category distance,
/// and within-category business distance, in that order.
pub struct ClassWeights {
    pub rows: [[f32; 5]; 3],
}

impl ClassWeights {
    pub fn from_parameters(source: &str, image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let mut rows = [[0.; 5]; 3];
        for (class, row) in rows.iter_mut().enumerate() {
            for (value, base) in row
                .iter_mut()
                .zip([0x53a6d8, 0x53a6e0, 0x53a6e4, 0x53a6dc, 0x53a6e8])
            {
                *value = image.f32_at(base + class as u32 * 0x54)?;
            }
        }
        for line in source.lines() {
            let fields: Vec<_> = line
                .split("//")
                .next()
                .unwrap()
                .split_whitespace()
                .collect();
            if !fields
                .first()
                .is_some_and(|s| s.eq_ignore_ascii_case("ClassWeighting"))
            {
                continue;
            }
            if fields.len() != 4 {
                return Err("non-literal ClassWeighting entry".into());
            }
            let Some(class) = ["low", "medium", "high"]
                .iter()
                .position(|s| fields[1].eq_ignore_ascii_case(s))
            else {
                continue;
            };
            let Some(metric) = ["value", "prestige", "quality", "distance", "microdistance"]
                .iter()
                .position(|s| fields[2].eq_ignore_ascii_case(s))
            else {
                continue;
            };
            let value = fields[3]
                .parse::<f64>()
                .map_err(|_| "non-literal class weight")? as f32;
            if !value.is_finite() {
                return Err("non-finite class weight".into());
            }
            rows[class][metric] = value;
        }
        Ok(Self { rows })
    }
}

/// One residence in original category linked-list order.
#[derive(Clone, Copy, Debug)]
pub struct HousingCandidate {
    pub residential: bool,
    pub capacity: u16,
    pub occupied: u16,
    /// Signed original building byte 2c; interpretation is not assumed here.
    pub attribute_2c: i8,
    pub rent: f32,
}

/// Inputs to original 0045a4b0, after resolving the resident's class and district.
#[derive(Clone, Copy, Debug)]
pub struct HousingScoreInputs {
    pub affordability_factor: f32,
    pub welfare: f32,
    pub rent_weight: f32,
    pub district_weight: f32,
    pub district_value: f32,
    /// Capacity-weighted citywide rent from 0047f970, not vacant-unit rent.
    pub average_rent: f32,
    pub current_category: bool,
}

/// Residential portion of 0047faa0. Supply active buildings in original slot
/// order, resolving each to its business; omit buildings without a business.
/// Original zero total capacity produces NaN, retained here for score clamping.
pub fn average_residential_rent<'a>(
    buildings: impl IntoIterator<Item = &'a HousingCandidate>,
) -> f32 {
    let mut weighted_rent = 0.0_f32;
    let mut capacity = 0_i32;
    for business in buildings {
        if business.residential {
            capacity = capacity.wrapping_add(i32::from(business.capacity));
            weighted_rent = (f64::from(business.rent) * f64::from(business.capacity)
                + f64::from(weighted_rent)) as f32;
        }
    }
    (f64::from(weighted_rent) / f64::from(capacity)) as f32
}

/// Original 0045a4b0 district score. `current` is the current building's index
/// within this category, if present. No qualifying capacity returns -99_999_999.
pub fn score_housing_category(
    candidates: &[HousingCandidate],
    current: Option<usize>,
    inputs: HousingScoreInputs,
) -> Result<i32, String> {
    let average = inputs.average_rent;
    score_housing_category_cached(candidates, current, inputs, || Ok(average))
}

///0045a4b0 calls the lazy rent getter only after finding qualifying capacity
/// and computing the district term. `inputs.average_rent` is unused here.
pub fn score_housing_category_cached(
    candidates: &[HousingCandidate],
    current: Option<usize>,
    inputs: HousingScoreInputs,
    mut average: impl FnMut() -> Result<f32, String>,
) -> Result<i32, String> {
    if [
        inputs.affordability_factor,
        inputs.welfare,
        inputs.rent_weight,
        inputs.district_weight,
        inputs.district_value,
    ]
    .iter()
    .any(|v| !v.is_finite())
        || current.is_some_and(|i| i >= candidates.len() || !inputs.current_category)
    {
        return Err("invalid housing category inputs".into());
    }
    let mut capacity = 0_i32;
    let mut weighted_rent = 0.0_f32;
    for (index, candidate) in candidates.iter().enumerate() {
        if !candidate.rent.is_finite() {
            return Err("non-finite housing rent".into());
        }
        let available = i32::from(candidate.capacity) - i32::from(candidate.occupied)
            + i32::from(current == Some(index));
        if !candidate.residential
            || available <= 0
            || f64::from(candidate.rent)
                > f64::from(inputs.affordability_factor) * f64::from(inputs.welfare)
        {
            continue;
        }
        capacity = capacity.wrapping_add(available);
        weighted_rent =
            (f64::from(available) * f64::from(candidate.rent) + f64::from(weighted_rent)) as f32;
    }
    if capacity == 0 {
        return Ok(-99_999_999);
    }
    let truncate = |value: f64| -> Result<i32, String> {
        let value = value.trunc();
        if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
            return Err("housing score outside supported integer range".into());
        }
        Ok(value as i32)
    };
    let district =
        truncate(f64::from(inputs.district_weight) * f64::from(inputs.district_value) * 40.0)?;
    let base = district.wrapping_add(if inputs.current_category {
        50
    } else {
        capacity.min(100)
    });
    // Native stores the scaled sum to f32 before dividing by retained average
    // times capacity. FCOM's unordered result also selects the zero clamp.
    let scaled_rent = (f64::from(weighted_rent) * 100.0) as f32;
    let average_rent = average()?;
    let rent_term =
        (200.0 - f64::from(scaled_rent) / (f64::from(average_rent) * f64::from(capacity))).max(0.0);
    truncate(rent_term * f64::from(inputs.rent_weight) + f64::from(base))
}

/// Original 0045a640 residence selection within one category.
/// `current` identifies the resident's existing building in this ordered slice.
pub fn select_residence(
    candidates: &[HousingCandidate],
    current: Option<usize>,
    affordability_factor: f32,
    welfare: f32,
    rent_weight: f32,
    attribute_weight: f32,
) -> Result<Option<usize>, String> {
    if [affordability_factor, welfare, rent_weight, attribute_weight]
        .iter()
        .any(|v| !v.is_finite())
        || current.is_some_and(|i| i >= candidates.len())
    {
        return Err("invalid housing selection inputs".into());
    }
    let truncate = |value: f64| -> Result<i32, String> {
        let value = value.trunc();
        if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
            return Err("housing score outside supported integer range".into());
        }
        Ok(value as i32)
    };
    let mut best = -99_999_999;
    let mut selected = None;
    for (index, candidate) in candidates.iter().enumerate() {
        if !candidate.rent.is_finite() {
            return Err("non-finite housing rent".into());
        }
        let is_current = current == Some(index);
        let capacity =
            i32::from(candidate.capacity) - i32::from(candidate.occupied) + i32::from(is_current);
        if !candidate.residential
            || capacity == 0
            || f64::from(candidate.rent) > f64::from(affordability_factor) * f64::from(welfare)
        {
            continue;
        }
        // Original selection accepts negative capacity and truncates the rent
        // contribution before adding it to the retained attribute contribution.
        let attribute =
            f64::from((i32::from(candidate.attribute_2c) + 1) * 33) * f64::from(attribute_weight);
        let rent = truncate(f64::from(candidate.rent) * f64::from(rent_weight))?;
        let mut score = truncate(attribute + f64::from(rent))?;
        if is_current {
            score = score.wrapping_add(50);
        }
        if score > best {
            best = score;
            selected = Some(index);
        }
    }
    Ok(selected)
}

#[derive(Clone, Copy, Debug)]
pub struct Preference {
    pub commodity: u32,
    pub factor: f32,
}

#[derive(Clone)]
pub struct Preferences {
    lists: [Vec<Preference>; 128],
}

/// Ordered association rows used by0045aed0/0045af80. These select a spending
/// preference list; they are distinct from that list's commodity weights.
pub struct PreferenceGroups {
    rows: [Vec<i8>; 15],
}

impl PreferenceGroups {
    pub fn empty() -> Self {
        Self {
            rows: std::array::from_fn(|_| Vec::new()),
        }
    }

    /// Original45aed0. Group15 appends to every row in order; a full later row
    /// returns false after earlier rows have already changed.
    pub fn associate(&mut self, group: i32, preference: i32) -> Result<bool, String> {
        if preference >= 128 {
            return Ok(false);
        }
        if !(0..=15).contains(&group) {
            return Err("preference group would address outside its native table".into());
        }
        let range = if group == 15 {
            0..15
        } else {
            group as usize..group as usize + 1
        };
        for index in range {
            let row = &mut self.rows[index];
            if row.len() >= 32 {
                return Ok(false);
            }
            row.push(preference as i8);
        }
        Ok(true)
    }

    pub fn associations(&self, group: usize) -> Option<&[i8]> {
        self.rows.get(group).map(Vec::as_slice)
    }

    pub fn from_v14_defaults(source: &str) -> Result<Self, String> {
        let mut rows: [Vec<i8>; 15] = std::array::from_fn(|_| Vec::new());
        let prefix = "InitialValue.AssociatePreference(";
        let lines: Vec<_> = source
            .lines()
            .map(|l| l.split("--").next().unwrap().trim())
            .collect();
        let first = lines
            .iter()
            .position(|l| l.starts_with(prefix))
            .ok_or("missing preference associations")?;
        let last = lines.iter().rposition(|l| l.starts_with(prefix)).unwrap();
        for &line in &lines[first..=last] {
            if line.is_empty() {
                continue;
            }
            let fields = line
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(");"))
                .ok_or("non-literal statement inside preference associations")?;
            let (category, preference) = fields
                .split_once(',')
                .ok_or("invalid preference association")?;
            let (class, age) = category
                .trim()
                .split_once("_CLASS_")
                .ok_or("invalid preference category")?;
            let class = ["LOW", "MEDIUM", "HIGH"]
                .iter()
                .position(|&v| v == class)
                .ok_or("unknown preference class")?;
            let age = ["CHILD", "TEENAGER", "ADULT", "PENSIONER", "TOURIST"]
                .iter()
                .position(|&v| v == age)
                .ok_or("unknown preference age")?;
            let value = preference
                .trim()
                .parse::<i8>()
                .map_err(|_| "invalid preference ID")?;
            let row = &mut rows[age * 3 + class];
            if value < 0 || row.len() == 32 {
                return Err("invalid preference association row".into());
            }
            row.push(value);
        }
        if rows.iter().any(Vec::is_empty) {
            return Err("empty preference association row".into());
        }
        Ok(Self { rows })
    }

    /// Selection portions of0045bb80 and0045c750. -1 requests a random field.
    /// Valid explicit ages/classes/preferences bypass their corresponding RNG.
    /// This does not allocate a resident or advance the separate initializer RNG.
    pub fn resolve(
        &self,
        requested: [i32; 3],
        age_class_rng: &mut u32,
        preference_rng: &mut u32,
    ) -> Result<[i32; 3], String> {
        let [mut age, mut class, mut preference] = requested;
        if !(-1..=4).contains(&age)
            || !(-1..=2).contains(&class)
            || !(-1..=127).contains(&preference)
        {
            return Err("unsupported resident identity input".into());
        }
        let next = |state: &mut u32| {
            *state = state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            (*state >> 16) & 0x7fff
        };
        if age == -1 {
            age = (next(age_class_rng) % 5) as i32;
        }
        if class == -1 {
            class = (next(age_class_rng) % 3) as i32;
        }
        if preference == -1 {
            let row = &self.rows[(age * 3 + class) as usize];
            preference = i32::from(row[next(preference_rng) as usize % row.len()]);
        }
        Ok([age, class, preference])
    }
}

/// Daily timetable choices in the insertion order used by original 00460b90.
/// Selector order is adult, unhoused, senior, schoolchild, university, tourist.
pub struct DailyObjectives {
    rows: [Vec<i32>; 6],
}

/// Original 00460610 bindings, indexed by daily objective and clock hour.
pub struct Timetables {
    rows: [[i32; 24]; 100],
}

impl Timetables {
    pub fn from_timetable(source: &str) -> Result<Self, String> {
        let mut rows = [[-1; 24]; 100];
        for line in source.lines() {
            let mut fields = line.split("//").next().unwrap().split_whitespace();
            if !fields
                .next()
                .is_some_and(|s| s.eq_ignore_ascii_case("TIMETABLE"))
            {
                continue;
            }
            let number = fields
                .next()
                .ok_or("missing timetable number")?
                .parse::<usize>()
                .map_err(|_| "invalid timetable number")?;
            let hour_name = fields
                .next()
                .ok_or("missing timetable hour")?
                .to_ascii_uppercase();
            let hour = hour_name
                .strip_prefix("HOUR")
                .ok_or("invalid timetable hour")?
                .parse::<usize>()
                .map_err(|_| "invalid timetable hour")?;
            if !(1..=24).contains(&hour) || hour_name != format!("HOUR{hour}") {
                return Err("invalid timetable hour".into());
            }
            let action = match fields
                .next()
                .ok_or("missing timetable action")?
                .to_ascii_uppercase()
                .as_str()
            {
                "SLEEP" => 0,
                "STROLL" => 1,
                "GO_HOME" => 2,
                "GO_TO_WORK" => 3,
                "GO_TO_SCHOOL" | "GO_TO_UNI" => 4,
                "LOOK_FOR_A_HOUSE" => 5,
                "LOOK_FOR_A_JOB" => 6,
                "COLLECT_WELFARE" => 7,
                "COLLECT_PENSION" => 8,
                "SNACK_TIME" => 9,
                "MEAL_TIME" => 10,
                "ENTERTAINMENT" => 11,
                "MAIN_SHOPPING" => 12,
                "STAY_OVER" => 13,
                "AT_HOME" => 14,
                "AT_WORK" | "AT_SCHOOL" | "AT_UNI" => 15,
                "UNDEFINED" => 17,
                _ => return Err("unknown timetable action".into()),
            };
            if fields.next().is_some() {
                return Err("extra timetable operand".into());
            }
            // HOUR24 names midnight. Repeated bindings overwrite in source order.
            rows.get_mut(number)
                .ok_or("timetable number out of range")?[hour % 24] = action;
        }
        Ok(Self { rows })
    }

    /// Raw dispatch ID; -1 is an unassigned cell, distinct from UNDEFINED (17).
    pub fn action(&self, objective: usize, hour: usize) -> Option<i32> {
        self.rows.get(objective)?.get(hour).copied()
    }
}

impl DailyObjectives {
    /// Read literal Add_Daily_Objective statements without evaluating settings.
    pub fn from_timetable(source: &str) -> Result<Self, String> {
        let mut rows: [Vec<i32>; 6] = std::array::from_fn(|_| Vec::new());
        for line in source.lines() {
            let mut fields = line.split("//").next().unwrap().split_whitespace();
            if !fields
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("Add_Daily_Objective"))
            {
                continue;
            }
            let value = fields
                .next()
                .ok_or("missing daily objective")?
                .parse::<i32>()
                .map_err(|_| "invalid daily objective")?;
            let category = fields.next().ok_or("missing daily objective category")?;
            let selector = match category.to_ascii_uppercase().as_str() {
                "ADULT" => 0,
                "UNHOUSED" => 1,
                "SENIOR_CITIZENS" => 2,
                "CHILDREN_SCHOOL" => 3,
                "TEENAGER_UNI" => 4,
                "TOURIST" => 5,
                _ => return Err("unknown daily objective category".into()),
            };
            if fields.next().is_some() || value >= 100 || rows[selector].len() == 32 {
                return Err("invalid daily objective statement".into());
            }
            // Native binding compares signed value < 100; duplicates and
            // negative values are not removed or sorted by the binding.
            rows[selector].push(value);
        }
        if rows.iter().any(Vec::is_empty) {
            return Err("missing daily objective category".into());
        }
        Ok(Self { rows })
    }

    pub fn get(&self, selector: usize) -> Option<&[i32]> {
        self.rows.get(selector).map(Vec::as_slice)
    }
}

pub(crate) fn commodity_names<'a>(
    image: &'a crate::pe::PeImage<'_>,
) -> Result<std::collections::BTreeMap<&'a str, u8>, String> {
    let mut names = std::collections::BTreeMap::new();
    for id in 0..32u8 {
        let target = u32::from_le_bytes(
            image
                .at(0x4706e0 + u32::from(id) * 4, 4)?
                .try_into()
                .unwrap(),
        );
        let code = image.at(target, 6)?;
        if code[0] != 0xb8 || code[5] != 0xc3 {
            return Err("unsupported native commodity name switch".into());
        }
        let address = u32::from_le_bytes(code[1..5].try_into().unwrap());
        let bytes = image.at(address, 64)?;
        let end = bytes
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated commodity name")?;
        let name = std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid commodity name")?;
        if names.insert(name, id).is_some() {
            return Err("duplicate native commodity name".into());
        }
    }
    Ok(names)
}

/// Apply literal original commodity membership statements to existing masks.
/// Order: snack, main shopping, entertainment, dining out, stay-over.
/// This preserves existing bits; it does not infer process or save initialization.
pub fn apply_commodity_memberships(
    source: &str,
    image: &crate::pe::PeImage<'_>,
    initial: [u32; 5],
) -> Result<[u32; 5], String> {
    let names = commodity_names(image)?;
    let mut masks = initial;
    for line in source.lines() {
        let line = line.split("--").next().unwrap().trim();
        if !line.contains(":Membership(") {
            continue;
        }
        let (name, category) = line
            .strip_prefix("Commodity[")
            .and_then(|s| s.strip_suffix(");"))
            .and_then(|s| s.split_once("]:Membership("))
            .ok_or("non-literal commodity membership")?;
        let id = *names
            .get(name.trim())
            .ok_or("unknown commodity membership name")?;
        let index = match category.trim() {
            "SNACKSHOPPING" => 0,
            "MAINSHOPPING" => 1,
            "ENTERTAINMENTSHOPPING" => 2,
            "DININGOUTSHOPPING" => 3,
            "STAYOVERSHOPPING" => 4,
            _ => return Err("unknown commodity membership category".into()),
        };
        masks[index] |= 1u32 << id;
    }
    Ok(masks)
}

impl Preferences {
    pub fn empty() -> Self {
        Self {
            lists: std::array::from_fn(|_| Vec::new()),
        }
    }

    /// Original45ae80 for in-bounds storage. Commodity words retain all32 bits.
    /// Negative list indices and overflowing native rows are explicit errors.
    pub fn append(&mut self, index: i32, commodity: i32, factor: f32) -> Result<bool, String> {
        if index >= 128 {
            return Ok(false);
        }
        let index = usize::try_from(index).map_err(|_| "negative preference list index")?;
        let commodity = commodity as u32;
        let list = &mut self.lists[index];
        if list.len() >= 32 {
            return Err("preference append would overwrite adjacent native storage".into());
        }
        list.push(Preference { commodity, factor });
        Ok(true)
    }

    /// Extract the contiguous literal AddPreference table in v1.4 DEFAULT.
    /// Other settings are not evaluated. IDs come from original 00470600's
    /// jump-table string operands rather than an assumed script enum order.
    pub fn from_v14_defaults(source: &str, image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let names = commodity_names(image)?;
        let lines: Vec<_> = source
            .lines()
            .map(|line| line.split("--").next().unwrap().trim())
            .collect();
        let prefix = "InitialValue.AddPreference(";
        let first = lines
            .iter()
            .position(|line| line.starts_with(prefix))
            .ok_or("missing preference table")?;
        let last = lines
            .iter()
            .rposition(|line| line.starts_with(prefix))
            .unwrap();
        let mut lists: [Vec<Preference>; 128] = std::array::from_fn(|_| Vec::new());
        for &line in &lines[first..=last] {
            if line.is_empty() {
                continue;
            }
            let values = line
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(");"))
                .ok_or("non-literal statement inside preference table")?;
            let fields: Vec<_> = values.split(',').map(str::trim).collect();
            if fields.len() != 3 {
                return Err("preference requires index, commodity and factor".into());
            }
            let index = fields[0]
                .parse::<usize>()
                .map_err(|_| "invalid preference index")?;
            let commodity = *names.get(fields[1]).ok_or("unknown preference commodity")?;
            let factor = fields[2]
                .parse::<f32>()
                .map_err(|_| "invalid preference factor")?;
            let list = lists
                .get_mut(index)
                .ok_or("preference index exceeds signed-byte range")?;
            if list.len() == 32 || !factor.is_finite() {
                return Err("invalid preference table entry".into());
            }
            list.push(Preference {
                commodity: u32::from(commodity),
                factor,
            });
        }
        Ok(Self { lists })
    }

    pub fn get(&self, index: i8) -> Option<&[Preference]> {
        self.lists
            .get(usize::try_from(index).ok()?)
            .map(Vec::as_slice)
    }
}

/// Serialized resident fields remain authoritative, including unknown bytes.
/// Runtime pointers are not transplanted from the original process.
#[derive(Clone, Debug)]
pub struct Resident {
    bytes: [u8; 0x14a],
    active: u8,
    auxiliary_reference: u32,
}

/// Original resident animation frame ranges,14 actions for each age model.
pub struct ResidentAnimationSettings {
    pub ranges: [[[u16; 2]; 14]; 5],
}
impl ResidentAnimationSettings {
    pub fn from_image(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let mut ranges = [[[0; 2]; 14]; 5];
        for (row, base) in ranges
            .iter_mut()
            .zip([0x53a69c, 0x53a62c, 0x53a5f4, 0x53a664, 0x53a5f4])
        {
            for (action, pair) in row.iter_mut().enumerate() {
                let bytes = image.at(base + action as u32 * 4, 4)?;
                *pair = [
                    u16::from_le_bytes(bytes[..2].try_into().unwrap()),
                    u16::from_le_bytes(bytes[2..].try_into().unwrap()),
                ];
            }
        }
        Ok(Self { ranges })
    }
}

/// Original9456a0 resident animation controller pool. References in records
/// remain opaque32-bit handles; they are never dereferenced as Rust pointers.
pub struct ResidentAnimationPool {
    records: Vec<[u8; 44]>,
}
pub struct ResidentAnimationRequest {
    pub action: u32,
    pub resident_reference: u32,
    pub auxiliary_reference: u32,
    pub model_frame_count: u16,
}
impl Default for ResidentAnimationPool {
    fn default() -> Self {
        Self {
            records: vec![[0; 44]; 2120],
        }
    }
}
impl ResidentAnimationPool {
    pub fn from_records(records: Vec<[u8; 44]>) -> Result<Self, String> {
        if records.len() > 2120 {
            return Err("animation pool exceeds original capacity".into());
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[[u8; 44]] {
        &self.records
    }

    /// Original004ece30 visits occupied slots in pool order. Dispatch remains
    /// explicit because non-resident controllers share this same pool. Later
    /// slots see changes made by earlier callbacks; a short pool represents
    /// unused trailing slots, not a compacted active list.
    pub fn advance(
        &mut self,
        mut controller: impl FnMut(usize, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        for index in 0..self.records.len() {
            if self.records[index][4] != 0 {
                controller(index, self)?;
            }
        }
        Ok(())
    }
}

/// Age-dependent movement speed and random displacement configuration.
pub struct MovementSettings {
    pub speed_by_age: [f32; 5],
    pub variation: f32,
}

impl MovementSettings {
    /// Original PeopleSpeed callback00438350, with untouched image defaults.
    /// This reads literal parameters without executing the parameter file.
    pub fn from_parameters(source: &str, image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let mut values = [0.; 6];
        for (i, value) in values.iter_mut().enumerate() {
            *value = image.f32_at(0x53a5c0 + i as u32 * 4)?;
        }
        for line in source.lines() {
            let fields: Vec<_> = line
                .split("//")
                .next()
                .unwrap()
                .split_whitespace()
                .collect();
            if !fields
                .first()
                .is_some_and(|s| s.eq_ignore_ascii_case("PeopleSpeed"))
            {
                continue;
            }
            if fields.len() != 3 {
                return Err("non-literal PeopleSpeed parameter".into());
            }
            let Some(index) = [
                "child",
                "teenager",
                "adult",
                "oap",
                "tourist",
                "displacement",
            ]
            .iter()
            .position(|name| fields[1].eq_ignore_ascii_case(name)) else {
                continue;
            };
            let value = fields[2]
                .parse::<f64>()
                .map_err(|_| "non-literal PeopleSpeed value")? as f32;
            if !value.is_finite() {
                return Err("non-finite PeopleSpeed value".into());
            }
            values[index] = value;
        }
        Ok(Self {
            speed_by_age: values[..5].try_into().unwrap(),
            variation: values[5],
        })
    }
}

/// Independent original streams used after age/class/preference resolution.
pub struct InitializationRng {
    pub constructor: u32,
    pub movement: u32,
    pub objective: u32,
}

/// Runtime inputs for0045c750. Speed rows and variation are configuration,
/// not welfare payments; the resulting field is consumed by movement.
pub struct InitializationInputs<'a> {
    pub movement: &'a MovementSettings,
    pub preferences: &'a Preferences,
    pub objectives: &'a DailyObjectives,
    pub timetables: &'a Timetables,
    pub commodities: &'a [Commodity; 32],
    pub commodity_masks: [u32; 5],
    pub hour: usize,
}

/// Owned resident configuration recovered from supplied settings, independent
/// of save files. Live commodity values/memberships and clock stay world-owned.
pub struct Configuration {
    pub movement: MovementSettings,
    /// Static signed edge threshold0053a814 read by original0045df80.
    pub movement_boundary: i32,
    pub class_weights: ClassWeights,
    pub preferences: Preferences,
    pub preference_groups: PreferenceGroups,
    pub objectives: DailyObjectives,
    pub timetables: Timetables,
}

impl Configuration {
    /// Prepare parameter/timetable data before live DEFAULT Lua settings run.
    /// Preference and association storage starts empty; script callbacks own it.
    pub fn before_defaults(
        parameters: &str,
        timetable: &str,
        image: &crate::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            movement: MovementSettings::from_parameters(parameters, image)?,
            movement_boundary: i32::from_le_bytes(image.at(0x53a814, 4)?.try_into().unwrap()),
            class_weights: ClassWeights::from_parameters(parameters, image)?,
            preferences: Preferences::empty(),
            preference_groups: PreferenceGroups::empty(),
            objectives: DailyObjectives::from_timetable(timetable)?,
            timetables: Timetables::from_timetable(timetable)?,
        })
    }

    pub fn from_sources(
        defaults: &str,
        parameters: &str,
        timetable: &str,
        image: &crate::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            movement: MovementSettings::from_parameters(parameters, image)?,
            movement_boundary: i32::from_le_bytes(image.at(0x53a814, 4)?.try_into().unwrap()),
            class_weights: ClassWeights::from_parameters(parameters, image)?,
            preferences: Preferences::from_v14_defaults(defaults, image)?,
            preference_groups: PreferenceGroups::from_v14_defaults(defaults)?,
            objectives: DailyObjectives::from_timetable(timetable)?,
            timetables: Timetables::from_timetable(timetable)?,
        })
    }

    pub fn inputs<'a>(
        &'a self,
        commodities: &'a [Commodity; 32],
        commodity_masks: [u32; 5],
        hour: usize,
    ) -> InitializationInputs<'a> {
        InitializationInputs {
            movement: &self.movement,
            preferences: &self.preferences,
            objectives: &self.objectives,
            timetables: &self.timetables,
            commodities,
            commodity_masks,
            hour,
        }
    }
}

/// Direct boundaries inside original00460470. Movement returns true when done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickEvent {
    Movement,
    RefreshIntent,
    Action,
}

/// Synchronous boundaries of the original street-crossing sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossingEvent {
    Prepare,
    Gate,
    Animation(u32),
    Begin,
    Finish,
    WaitingAnimation,
}

/// Original category crossing links, road neighbors and32-byte traffic rows.
pub struct CrossingWorld<'a> {
    pub category_links: &'a [[u8; 4]; 52],
    pub road_neighbors: &'a [[u8; 4]],
    pub lane_base: u32,
    pub lanes: &'a mut [[u8; 32]],
}

pub struct CategoryTravelState<'a, 'w> {
    pub centers: &'a [[f32; 2]; 52],
    pub boundary: i32,
    pub category_counts: &'a mut [u32; 52],
    pub routes: &'a mut crate::routing::CategoryRoutes,
    pub crossing: &'a mut CrossingWorld<'w>,
}

/// Shared runtime inputs for the no-housing roaming action and street travel.
pub struct RoamingState<'a, 'w> {
    pub travel: CategoryTravelState<'a, 'w>,
    pub category_enabled: &'a [u8; 52],
    pub rng: &'a mut [u32; 3],
}

/// Deep operations invoked by the original housing action dispatcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HousingAction {
    Choose,
    TravelA,
    TravelB,
    Assign,
}

/// Original direct simulation helpers, named by address until reconstructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ActionHelper {
    At45abd0 = 0x45abd0,
    At45ac90 = 0x45ac90,
    At45d560 = 0x45d560,
    At45e080 = 0x45e080,
    At45f660 = 0x45f660,
    At45fa40 = 0x45fa40,
    At45f2e0 = 0x45f2e0,
    At45ecb0 = 0x45ecb0,
    At45e830 = 0x45e830,
    At45e680 = 0x45e680,
    At45e430 = 0x45e430,
    At45f950 = 0x45f950,
    At45f1c0 = 0x45f1c0,
}

/// Resolved runtime inputs for0045f660. Preference limit is indexed by
/// signed preference row and catalog slot, not by commodity ID.
#[derive(Clone, Copy)]
pub struct RetailInputs {
    pub slot: usize,
    pub preference_limit: f32,
    pub interaction_delay: f32,
    pub unit_cost: f32,
    pub tick: u32,
}

/// Original enabled-building mapping and business/commodity settings used by
///0045f660. None means disabled or unmapped, both native early exits.
pub struct RetailWorld<'a> {
    pub building_businesses: &'a [Option<usize>],
    pub catalogs: &'a [Vec<u32>],
    pub interaction_delays: &'a [f32],
    pub unit_costs: &'a [f32; 32],
    pub preferences: &'a Preferences,
    pub tick: u32,
}

/// Mutable city state needed to travel to and buy from a selected local business.
pub struct LocalPurchaseState<'a, 'w> {
    pub postings: &'a mut Vec<crate::economy::AccountPosting>,
    pub retail: &'a RetailWorld<'w>,
    pub centers: &'a [[f32; 2]; 52],
    pub boundary: i32,
    pub category_counts: &'a mut [u32; 52],
    pub businesses: &'a mut [[u8; 544]],
    pub accounts: &'a mut [SpendingAccount],
    pub owners: &'a mut [crate::economy::PlayerAccount],
}

/// Static destination data; eligibility masks are read from live businesses.
pub struct ShoppingNode {
    pub building: i32,
    pub position: [f32; 2],
}

/// Selection inputs shared across every category visited during a shopping trip.
/// Catalogs, centers and mutable ledgers come from `ShoppingState::local` so
/// decisions observe the same business/account state that transactions update.
pub struct ShoppingSelection<'a> {
    pub chains: &'a [Vec<Option<usize>>; 52],
    pub category_masks: &'a [u32; 52],
    pub category_factors: &'a [f32; 52],
    pub building_categories: &'a [i16],
    pub building_attributes: &'a [i8],
    pub commodities: &'a [Commodity; 32],
    pub weights: &'a [[f32; 4]],
    pub distance_disabled: bool,
    pub category_owners: &'a [Option<usize>; 52],
    pub nodes: &'a [Vec<ShoppingNode>; 52],
    pub entrances: &'a [[f32; 3]],
    pub business_distance_weight: f32,
}

/// Shared state for selection, route discovery, crossing, local travel and sale.
/// Animation-manager timing and non-shopping tick events remain explicit.
pub struct ShoppingState<'a, 'w> {
    pub local: LocalPurchaseState<'a, 'w>,
    pub selection: ShoppingSelection<'a>,
    pub routes: &'a mut crate::routing::CategoryRoutes,
    pub crossing: &'a mut CrossingWorld<'w>,
}

/// Resolved original commodity row, building byte2c and resident settings row.
#[derive(Clone, Copy, Debug)]
pub struct PurchaseCandidateInputs {
    pub commodity: usize,
    pub value: f32,
    pub markup: f32,
    pub building_attribute: i8,
    /// Original53a6d8,53a6e0,53a6e4, with resident3c row stride54.
    pub weights: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PurchaseCandidateScore {
    pub value: f64,
    pub ratio: Option<f32>,
    pub category_factor: Option<f32>,
}

/// Resolved category-list node used by0045ac90, preserving original list order.
pub struct PurchaseNode {
    pub building: i32,
    pub position: [f32; 2],
    pub business_mask: Option<u32>,
}

pub struct PurchaseBusinessInputs<'a> {
    pub preferences: &'a [u32],
    pub account_mask: u32,
    pub nodes: &'a [PurchaseNode],
    pub auxiliary_position: [f32; 2],
    pub distance_weight: f32,
}

/// Inputs for the two recovered decision stages inside the purchase action.
pub struct PurchaseDecisions<'a, 'w> {
    pub world: &'a PurchaseWorld<'w>,
    pub accounts: &'a [SpendingAccount],
    pub category_owners: &'a [Option<usize>; 52],
    pub preferences: &'a [u32],
    /// Current category7c nodes, in world.chains order.
    pub nodes: &'a [PurchaseNode],
    pub entrances: &'a [[f32; 3]],
    pub business_distance_weight: f32,
}

/// Borrowed simulation inputs for a complete category choice. Chains preserve
/// original node order, including unresolved nodes; catalogs preserve script order.
pub struct PurchaseWorld<'a> {
    pub businesses: &'a [[u8; 0x220]],
    pub catalogs: &'a [Vec<u32>],
    pub chains: &'a [Vec<Option<usize>>; 52],
    pub category_masks: &'a [u32; 52],
    pub category_factors: &'a [f32; 52],
    pub centers: &'a [[f32; 2]; 52],
    pub building_categories: &'a [i16],
    pub building_attributes: &'a [i8],
    pub commodities: &'a [Commodity; 32],
    /// Ratio, category factor, map attribute, distance weights by resident3c.
    pub weights: &'a [[f32; 4]],
    pub distance_disabled: bool,
}

/// Original00462960 after resolving the business type's ordered catalog to
/// commodity identities. A missing identity reads the native fallback at1c0.
pub fn purchase_price(
    business: &[u8; 0x220],
    catalog: &[u32],
    commodity: u32,
) -> Result<f32, String> {
    if catalog.len() > 6 {
        return Err("purchase catalog exceeds six business slots".into());
    }
    let at = catalog
        .iter()
        .position(|&id| id == commodity)
        .map_or(0x1c0, |slot| 0x1c4 + slot * 4);
    Ok(f32::from_le_bytes(business[at..at + 4].try_into().unwrap()))
}

/// Original00462f30 with host-neutral building categories. Preserve signed
/// AX and the explicit -1 sentinel; other invalid indices are errors.
pub fn purchase_business_category(
    business: &[u8; 0x220],
    categories: &[i16],
) -> Result<i16, String> {
    let index = i16::from_le_bytes(business[8..10].try_into().unwrap());
    if index == -1 {
        return Ok(-1);
    }
    usize::try_from(index)
        .ok()
        .and_then(|i| categories.get(i))
        .copied()
        .ok_or_else(|| "purchase building leaves category table".into())
}

const HEADING_RANK: [u8; 16] = [
    255, 0, 2, 1, 4, 255, 3, 255, 6, 7, 255, 255, 5, 255, 255, 255,
];

fn step_heading(current: u32, positive: bool) -> u32 {
    if positive {
        match current {
            1 => 3,
            2 => 6,
            3 => 2,
            4 => 12,
            6 => 4,
            8 => 9,
            9 => 1,
            12 => 8,
            _ => current,
        }
    } else {
        match current {
            1 => 9,
            2 => 3,
            3 => 1,
            4 => 6,
            6 => 2,
            8 => 12,
            9 => 8,
            12 => 4,
            _ => current,
        }
    }
}

fn heading_position(heading: u32, displacement: i8) -> [f32; 3] {
    let m = f32::from(displacement) + 35.;
    [
        match heading {
            2 | 3 | 6 => m,
            8 | 9 | 12 => -m,
            _ => 0.,
        },
        0.5,
        match heading {
            1 | 3 | 9 => m,
            4 | 6 | 12 => -m,
            _ => 0.,
        },
    ]
}

/// Original0045df80: perimeter and sign heading masks, in output-pointer order.
pub fn position_headings(position: [f32; 3], boundary: i32) -> [u32; 2] {
    let mut edge = 0;
    let mut sign = 0;
    for (value, positive, negative) in [(position[0], 2, 8), (position[2], 1, 4)] {
        let value = f64::from(value);
        if value >= f64::from(boundary) || value.is_nan() {
            edge |= positive;
            sign |= positive;
        } else if value <= f64::from(boundary.wrapping_neg()) {
            edge |= negative;
            sign |= negative;
        } else if value > 0. {
            sign |= positive;
        } else if value < 0. {
            sign |= negative;
        }
    }
    if edge == 0 {
        edge = sign;
    }
    if sign == 0 {
        sign = edge;
    }
    [edge, sign]
}

impl Resident {
    /// Original0045cb70 increments the packed two-bit category counter,
    /// saturating at3. Preserve signed division and x86 shift counts for the
    /// signed category word passed by the business binding.
    pub fn invalidate_category(&mut self, category: i16) -> Result<(), String> {
        let category = i32::from(category);
        let offset = usize::try_from(0x2c + category / 4)
            .map_err(|_| "category counter precedes resident")?;
        let byte = self
            .bytes
            .get_mut(offset)
            .ok_or("category counter leaves resident")?;
        let shift = (category % 4 * 2) as u32;
        let mask = 3_i32.wrapping_shl(shift);
        let current = (i32::from(*byte as i8) & mask).wrapping_shr(shift);
        let next = (current + 1).min(3);
        // x86 byte shifts mask CL to five bits, not to the three bits used
        // by Rust's u8 wrapping_shl; narrow after a32-bit shift instead.
        *byte = (*byte & !(mask as u8)) | ((next as u32 & 3).wrapping_shl(shift) as u8);
        Ok(())
    }
    /// Original45c170 on the category-counter array at resident+2c, with
    /// value0. Event-card effect6 resets this even for inactive pool slots.
    pub fn reset_category_counter(&mut self, category: i32) -> Result<(), String> {
        let offset = usize::try_from(0x2ci32.wrapping_add(category / 4))
            .map_err(|_| "category counter precedes resident")?;
        let byte = self
            .bytes
            .get_mut(offset)
            .ok_or("category counter leaves resident")?;
        let shift = (category % 4 * 2) as u32;
        *byte &= !(3u32.wrapping_shl(shift) as u8);
        Ok(())
    }
    /// One004ece30 manager visit to a resident controller. Call in pool-index
    /// order at the host's animation-manager boundary, separately from ticks.
    pub fn advance_animation_slot(
        &mut self,
        auxiliary: &mut [u8; 284],
        pool: &mut ResidentAnimationPool,
        index: usize,
        resident_reference: u32,
        auxiliary_reference: u32,
    ) -> Result<(), String> {
        let record = pool
            .records
            .get(index)
            .ok_or("animation slot outside pool")?;
        if record[4] == 0 {
            return Ok(());
        }
        let kind = u32::from_le_bytes(record[..4].try_into().unwrap());
        if kind > 3 {
            return Err("animation controller is not a resident frame sequence".into());
        }
        if u32::from_le_bytes(record[8..12].try_into().unwrap()) != auxiliary_reference {
            return Err("animation slot belongs to another auxiliary".into());
        }
        let start = u16::from_le_bytes(record[32..34].try_into().unwrap());
        let end = u16::from_le_bytes(record[34..36].try_into().unwrap());
        let frame = u16::from_le_bytes(auxiliary[0x2c..0x2e].try_into().unwrap());
        let frame = if kind == 0 || kind == 2 {
            frame.wrapping_add(1)
        } else {
            frame.wrapping_sub(1)
        };
        auxiliary[0x2c..0x2e].copy_from_slice(&frame.to_le_bytes());
        auxiliary[0xb8] |= 8;
        let reached = if kind == 0 || kind == 2 {
            i32::from(frame as i16) >= i32::from(end)
        } else {
            i32::from(frame as i16) == i32::from(end)
        };
        if reached {
            if kind < 2 {
                auxiliary[0x2c..0x2e].copy_from_slice(&start.to_le_bytes());
            } else {
                self.release_animation_slot(
                    auxiliary,
                    pool,
                    index,
                    resident_reference,
                    auxiliary_reference,
                )?;
            }
        }
        Ok(())
    }

    /// Original0045e830 fallback travel. The original transfers the embedded
    /// movement origin to the destination category during phase8, then turns
    /// and schedules its final approach; preserve that distinct path.
    pub fn advance_fallback_travel(
        &mut self,
        auxiliary: &mut [u8; 284],
        boundary: i32,
        centers: &[[f32; 2]; 52],
        counts: &mut [u32; 52],
    ) -> Result<bool, String> {
        let word = |r: &Self, at| u32::from_le_bytes(r.bytes[at..at + 4].try_into().unwrap());
        let put = |r: &mut Self, at, v: u32| r.bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let clear = |r: &mut Self| {
            for at in [0xd8, 0xe0, 0xe4] {
                r.bytes[at..at + 4].fill(0);
            }
        };
        let mut phase = word(self, 0xd4);
        if phase == 0 {
            put(self, 0x90, 0);
            put(self, 0x94, 0.5_f32.to_bits());
            put(self, 0x98, (-41_f32).to_bits());
            put(self, 0xd4, 6);
            clear(self);
            phase = 6;
        }
        match phase {
            6 => {
                if self.advance_entrance_approach(boundary, centers, |r| {
                    r.advance_perimeter(centers)
                })? {
                    put(self, 0xd4, 7);
                    clear(self);
                }
            }
            7 => {
                let inner = word(self, 0xd8);
                if inner == 0 {
                    put(self, 0x9c, 0);
                    put(self, 0xa4, word(self, 0x98));
                    put(self, 0xd8, 2);
                    put(self, 0xe0, 0);
                    put(self, 0xe4, 0);
                }
                if inner == 0 || inner == 2 {
                    self.schedule_local_movement(centers)?;
                    put(self, 0xd4, 8);
                    clear(self);
                }
            }
            8 => {
                let old = usize::from(self.bytes[0x7c]);
                let target = usize::from(self.bytes[0x7d]);
                if old >= 52 || target >= 52 {
                    return Err("fallback category outside table".into());
                }
                counts[old] = counts[old].wrapping_sub(1);
                self.bytes[0x7c] = target as u8;
                counts[target] = counts[target].wrapping_add(1);
                self.bytes[0x2c + target / 4] |= 3 << ((target % 4) * 2);
                let local_x = f64::from(f32::from_bits(word(self, 0xac)));
                let local_z = f64::from(f32::from_bits(word(self, 0xb4)));
                let position = [
                    (local_x + f64::from(centers[target][0]) + 470.) as f32,
                    (local_z + f64::from(centers[target][1]) + 450.) as f32,
                ];
                self.set_movement_origin(position);
                self.set_movement_target(position);
                clear(self);
                put(self, 0xd4, 9);
            }
            9 => {
                let yaw = f64::from(f32::from_le_bytes(
                    auxiliary[0x14..0x18].try_into().unwrap(),
                )) + 180.;
                auxiliary[0x14..0x18].copy_from_slice(&(yaw as f32).to_le_bytes());
                clear(self);
                put(self, 0xd4, 10);
                put(self, 0x90, 0);
                put(self, 0x94, 0.5_f32.to_bits());
                put(self, 0x98, (-38_f32).to_bits());
            }
            10 => {
                let inner = word(self, 0xd8);
                if inner == 0 {
                    self.bytes.copy_within(0x90..0x9c, 0x9c);
                    put(self, 0xd8, 2);
                    put(self, 0xe0, 0);
                    put(self, 0xe4, 0);
                }
                if inner == 0 || inner == 2 {
                    self.schedule_local_movement(centers)?;
                    put(self, 0x80, 4);
                    return Ok(true);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    /// Original0045b9f0 sets raw and masked origin X/Z while preserving raw Y.
    pub fn set_movement_origin(&mut self, position: [f32; 2]) {
        for (at, value) in [(0x124, position[0]), (0x128, 0.), (0x12c, position[1])] {
            let bits = value.to_bits();
            let shift = (bits.wrapping_sub(0x3b800000) >> 23) & 31;
            self.bytes[at..at + 4]
                .copy_from_slice(&(bits & (i32::MIN >> shift) as u32).to_le_bytes());
        }
        self.bytes[0xf4..0xf8].copy_from_slice(&position[0].to_le_bytes());
        self.bytes[0xfc..0x100].copy_from_slice(&position[1].to_le_bytes());
    }

    fn release_animation_slot(
        &mut self,
        auxiliary: &mut [u8; 284],
        pool: &mut ResidentAnimationPool,
        index: usize,
        resident_reference: u32,
        auxiliary_reference: u32,
    ) -> Result<(), String> {
        let old = pool
            .records
            .get_mut(index)
            .ok_or("animation handle outside pool")?;
        let word = |at| u32::from_le_bytes(old[at..at + 4].try_into().unwrap());
        if word(8) != auxiliary_reference {
            return Err("animation controller belongs to another auxiliary".into());
        }
        let kind = word(0);
        let callback = word(0x14);
        let argument = word(0x18);
        let end = u16::from_le_bytes(old[0x22..0x24].try_into().unwrap());
        if kind <= 3 {
            auxiliary[0x2c..0x2e].copy_from_slice(
                &if kind == 0 || kind == 2 {
                    end.wrapping_sub(1)
                } else {
                    end
                }
                .to_le_bytes(),
            );
        }
        auxiliary[0x2e..0x30].fill(0xff);
        if callback != 0 {
            if callback != 0x45c200 || argument != resident_reference {
                return Err("animation release requires another callback owner".into());
            }
            self.bytes[0xbc] = 1;
        }
        old[0x14..0x18].fill(0);
        old[4] = 0;
        Ok(())
    }

    /// Original0045ce10 and controller release/allocation/range setup. Releasing
    /// the preceding resident controller runs0045c200 before the new request.
    /// Model frame count is the resolved metadata word read by004eccc0.
    pub fn request_animation(
        &mut self,
        auxiliary: &mut [u8; 284],
        pool: &mut ResidentAnimationPool,
        settings: &ResidentAnimationSettings,
        request: ResidentAnimationRequest,
    ) -> Result<(), String> {
        let ResidentAnimationRequest {
            action,
            resident_reference,
            auxiliary_reference,
            model_frame_count,
        } = request;
        let handle = i16::from_le_bytes(auxiliary[0x2e..0x30].try_into().unwrap());
        if handle != -1 {
            self.release_animation_slot(
                auxiliary,
                pool,
                usize::try_from(handle).map_err(|_| "negative animation handle")?,
                resident_reference,
                auxiliary_reference,
            )?;
        }
        let index = pool
            .records
            .iter()
            .position(|r| r[4] == 0)
            .ok_or("animation pool exhausted")?;
        let record = &mut pool.records[index];
        record[4] = 1;
        for (at, value) in [
            (0, if action == 2 { 0_u32 } else { 2 }),
            (8, auxiliary_reference),
            (0x14, 0x45c200),
            (0x18, resident_reference),
            (0x1c, 0),
        ] {
            record[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        auxiliary[0x2e..0x30].copy_from_slice(&(index as u16).to_le_bytes());
        record[0x20..0x22].fill(0);
        record[0x22..0x24].copy_from_slice(&model_frame_count.to_le_bytes());
        if action != 2 {
            self.bytes[0xb8..0xbc].fill(0);
        }
        let age = u32::from_le_bytes(self.bytes[..4].try_into().unwrap()) as usize;
        if let Some(ranges) = settings.ranges.get(age) {
            let pair = ranges
                .get(action as usize)
                .ok_or("animation action outside frame table")?;
            record[0x20..0x22].copy_from_slice(&pair[0].to_le_bytes());
            record[0x22..0x24].copy_from_slice(&pair[1].to_le_bytes());
            auxiliary[0x2c..0x2e].copy_from_slice(&pair[0].to_le_bytes());
        }
        Ok(())
    }

    /// Tick-driven category travel with actual crossing occupancy and movement.
    /// Animation requests retain an explicit host-owned controller boundary.
    pub fn tick_with_category_travel(
        &mut self,
        auxiliary: &mut [u8; 284],
        state: &mut CategoryTravelState<'_, '_>,
        mut animation: impl FnMut(u32, &mut Self, &mut [u8; 284]) -> Result<(), String>,
        mut tick_helper: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut other: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick_with_purchase_actions(
            auxiliary,
            state.centers,
            |event, r, a| {
                if event == TickEvent::Movement {
                    Ok(r.advance_movement(a))
                } else {
                    tick_helper(event, r, a)
                }
            },
            |helper, r, a| {
                if helper == ActionHelper::At45ecb0 {
                    r.advance_category_route(a, state, &mut animation)
                } else if helper == ActionHelper::At45e830 {
                    r.advance_fallback_travel(
                        a,
                        state.boundary,
                        state.centers,
                        state.category_counts,
                    )
                } else {
                    other(helper, r, a)
                }
            },
        )
    }

    /// Tick home/roaming actions with live routing, crossing and constructor RNG.
    /// Special home arrivals post to the shared original property-income totals.
    pub fn tick_with_home_and_roaming(
        &mut self,
        auxiliary: &mut [u8; 284],
        state: &mut RoamingState<'_, '_>,
        mut animation: impl FnMut(u32, &mut Self, &mut [u8; 284]) -> Result<(), String>,
        mut tick_helper: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut other: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        properties: &mut crate::economy::PropertyIncome,
    ) -> Result<(), String> {
        self.tick(
            auxiliary,
            state.travel.centers,
            |event, resident, auxiliary| match event {
                TickEvent::Movement => Ok(resident.advance_movement(auxiliary)),
                TickEvent::Action => {
                    resident.advance_home_and_roaming_action(
                        auxiliary,
                        state,
                        &mut animation,
                        &mut other,
                        properties,
                    )?;
                    Ok(false)
                }
                _ => tick_helper(event, resident, auxiliary),
            },
        )
    }

    /// Action-only home/roaming dispatch for the shared population tick.
    pub fn advance_home_and_roaming_action(
        &mut self,
        auxiliary: &mut [u8; 284],
        state: &mut RoamingState<'_, '_>,
        mut animation: impl FnMut(u32, &mut Self, &mut [u8; 284]) -> Result<(), String>,
        mut other: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        properties: &mut crate::economy::PropertyIncome,
    ) -> Result<(), String> {
        let mut helper = |helper, resident: &mut Self, auxiliary: &mut [u8; 284]| {
            let mut travel = |helper, resident: &mut Self, auxiliary: &mut [u8; 284]| match helper {
                ActionHelper::At45ecb0 => {
                    resident.advance_category_route(auxiliary, &mut state.travel, &mut animation)
                }
                ActionHelper::At45e830 => resident.advance_fallback_travel(
                    auxiliary,
                    state.travel.boundary,
                    state.travel.centers,
                    state.travel.category_counts,
                ),
                ActionHelper::At45e680 => resident.advance_local_destination(
                    state.travel.boundary,
                    state.travel.centers,
                    state.travel.category_counts,
                ),
                ActionHelper::At45e430 => {
                    resident.advance_local_roaming(
                        state.travel.centers,
                        state.travel.category_counts,
                        |resident| animation(2, resident, auxiliary),
                        |resident| resident.advance_perimeter(state.travel.centers),
                    )?;
                    Ok(false)
                }
                _ => other(helper, resident, auxiliary),
            };
            if helper == ActionHelper::At45f1c0 {
                resident.advance_roaming_action(
                    auxiliary,
                    state.category_enabled,
                    state.rng,
                    &mut travel,
                )
            } else if helper == ActionHelper::At45f2e0 {
                resident.advance_home_action(auxiliary, state.rng, &mut travel, |index, amount| {
                    properties.record_arrival(index as usize, amount)
                })
            } else {
                travel(helper, resident, auxiliary)
            }
        };
        self.advance_action(auxiliary, |action, resident, auxiliary| {
            if action == ActionHelper::At45f950 {
                resident.advance_purchase_action(auxiliary, &mut helper)
            } else {
                helper(action, resident, auxiliary)
            }
        })
    }

    pub(crate) fn advance_category_route(
        &mut self,
        a: &mut [u8; 284],
        state: &mut CategoryTravelState<'_, '_>,
        mut animation: impl FnMut(u32, &mut Self, &mut [u8; 284]) -> Result<(), String>,
    ) -> Result<bool, String> {
        self.advance_category_travel(
            state.boundary,
            state.centers,
            state.category_counts,
            |from, to| state.routes.next_hop(from, to),
            |event, r| {
                match event {
                    CrossingEvent::Prepare => r.prepare_crossing(state.centers)?,
                    CrossingEvent::Gate => return r.crossing_allowed(state.crossing),
                    CrossingEvent::Begin => r.set_crossing_active(state.crossing, true)?,
                    CrossingEvent::Finish => r.set_crossing_active(state.crossing, false)?,
                    CrossingEvent::Animation(mode) => animation(mode, r, a)?,
                    CrossingEvent::WaitingAnimation => {
                        match u32::from_le_bytes(r.bytes[0xe4..0xe8].try_into().unwrap()) {
                            0 | 4 => {
                                animation(5, r, a)?;
                                r.bytes[0xe4..0xe8].copy_from_slice(&5_u32.to_le_bytes());
                            }
                            5 => animation(4, r, a)?,
                            _ => {}
                        }
                    }
                }
                Ok(false)
            },
        )
    }

    /// Run the complete recovered shopping action against one shared ledger.
    pub fn tick_with_shopping(
        &mut self,
        auxiliary: &mut [u8; 284],
        state: &mut ShoppingState<'_, '_>,
        mut animation: impl FnMut(u32, &mut Self, &mut [u8; 284]) -> Result<(), String>,
        mut tick_helper: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut other: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick(
            auxiliary,
            state.local.centers,
            |event, resident, auxiliary| match event {
                TickEvent::Movement => Ok(resident.advance_movement(auxiliary)),
                TickEvent::Action => {
                    resident.advance_shopping_action(
                        auxiliary,
                        state,
                        &mut animation,
                        &mut other,
                    )?;
                    Ok(false)
                }
                _ => tick_helper(event, resident, auxiliary),
            },
        )
    }

    /// Outer action dispatch without repeating movement/countdown gates. This
    /// can be called from ResidentAccounts::tick_resident while the resident
    /// remains in the authoritative pool alongside its occupancy chains.
    pub fn advance_shopping_action(
        &mut self,
        auxiliary: &mut [u8; 284],
        state: &mut ShoppingState<'_, '_>,
        mut animation: impl FnMut(u32, &mut Self, &mut [u8; 284]) -> Result<(), String>,
        mut other: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        let mut helper = |helper, r: &mut Self, a: &mut [u8; 284]| match helper {
            ActionHelper::At45ecb0 => r.advance_category_route(
                a,
                &mut CategoryTravelState {
                    centers: state.local.centers,
                    boundary: state.local.boundary,
                    category_counts: state.local.category_counts,
                    routes: state.routes,
                    crossing: state.crossing,
                },
                &mut animation,
            ),
            ActionHelper::At45e830 => r.advance_fallback_travel(
                a,
                state.local.boundary,
                state.local.centers,
                state.local.category_counts,
            ),
            ActionHelper::At45e680 => r.advance_local_destination(
                state.local.boundary,
                state.local.centers,
                state.local.category_counts,
            ),
            ActionHelper::At45f660 => {
                r.purchase_selected_with_postings(
                    state.local.retail,
                    state.local.businesses,
                    state.local.accounts,
                    state.local.owners,
                    state.local.postings,
                )?;
                Ok(false)
            }
            ActionHelper::At45abd0 | ActionHelper::At45ac90 => {
                let selection = &state.selection;
                let world = PurchaseWorld {
                    businesses: state.local.businesses,
                    catalogs: state.local.retail.catalogs,
                    chains: selection.chains,
                    category_masks: selection.category_masks,
                    category_factors: selection.category_factors,
                    centers: state.local.centers,
                    building_categories: selection.building_categories,
                    building_attributes: selection.building_attributes,
                    commodities: selection.commodities,
                    weights: selection.weights,
                    distance_disabled: selection.distance_disabled,
                };
                let preferences = if helper == ActionHelper::At45ac90 {
                    state
                        .local
                        .retail
                        .preferences
                        .get(r.bytes[0x6a] as i8)
                        .ok_or("shopping preference row outside table")?
                        .iter()
                        .map(|entry| entry.commodity)
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                if helper == ActionHelper::At45ac90 && preferences.is_empty() {
                    return Ok(false);
                }
                let nodes = if helper == ActionHelper::At45ac90 {
                    let category = usize::from(r.bytes[0x7c]);
                    let nodes = selection
                        .nodes
                        .get(category)
                        .ok_or("shopping category outside node table")?;
                    let chain = &selection.chains[category];
                    if chain.len() != nodes.len() {
                        return Err("shopping node data differs from category chain".into());
                    }
                    chain
                        .iter()
                        .zip(nodes)
                        .map(|(entry, node)| {
                            let business_mask = entry
                                .map(|index| {
                                    world
                                        .businesses
                                        .get(index)
                                        .map(|b| {
                                            u32::from_le_bytes(b[0x70..0x74].try_into().unwrap())
                                        })
                                        .ok_or("shopping chain leaves business pool")
                                })
                                .transpose()?;
                            Ok(PurchaseNode {
                                building: node.building,
                                position: node.position,
                                business_mask,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?
                } else {
                    Vec::new()
                };
                r.purchase_decision(
                    helper,
                    a,
                    &PurchaseDecisions {
                        world: &world,
                        accounts: state.local.accounts,
                        category_owners: selection.category_owners,
                        preferences: &preferences,
                        nodes: &nodes,
                        entrances: selection.entrances,
                        business_distance_weight: selection.business_distance_weight,
                    },
                    state.local.owners,
                )
            }
            _ => other(helper, r, a),
        };
        self.advance_action(auxiliary, |action, resident, auxiliary| {
            if action == ActionHelper::At45f950 {
                resident.advance_purchase_action(auxiliary, &mut helper)
            } else {
                helper(action, resident, auxiliary)
            }
        })
    }

    /// Original0045d140: construct the crossing endpoint in current-category
    /// coordinates; the category itself changes only after movement finishes.
    pub fn prepare_crossing(&mut self, centers: &[[f32; 2]; 52]) -> Result<(), String> {
        let word = |at| u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap());
        let heading = word(0xa8);
        let current = word(0x80);
        let negative = current
            & match heading {
                1 => 8,
                2 => 1,
                4 => 2,
                8 => 4,
                _ => 0,
            }
            != 0;
        let next = step_heading(step_heading(current, !negative), !negative);
        let target =
            usize::try_from(self.bytes[0x8d] as i8).map_err(|_| "negative crossing target")?;
        let target = centers
            .get(target)
            .ok_or("crossing target outside centers")?;
        let source = centers
            .get(usize::from(self.bytes[0x7c]))
            .ok_or("crossing source outside centers")?;
        let mut position = heading_position(next, self.bytes[0x8c] as i8);
        position[0] = (f64::from(target[0]) - f64::from(source[0]) + f64::from(position[0])) as f32;
        position[2] = (f64::from(target[1]) - f64::from(source[1]) + f64::from(position[2])) as f32;
        self.bytes[0x84..0x88].copy_from_slice(&next.to_le_bytes());
        for (at, value) in [0x9c, 0xa0, 0xa4].into_iter().zip(position) {
            self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        Ok(())
    }

    fn crossing_lane(&self, world: &CrossingWorld<'_>) -> Result<Option<usize>, String> {
        let word = |at| u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap());
        let heading = word(0xa8);
        let link = match heading {
            1 => 0,
            2 => 2,
            4 => 1,
            8 => 3,
            _ => return Err("non-cardinal crossing heading".into()),
        };
        let category = world
            .category_links
            .get(usize::from(self.bytes[0x7c]))
            .ok_or("crossing category missing")?;
        let junction = category[link];
        if junction == 255 {
            return Ok(None);
        }
        let record = world
            .road_neighbors
            .get(usize::from(junction))
            .ok_or("crossing road missing")?;
        let slot = match word(0x80) & !heading {
            1 => 0,
            4 => 1,
            2 => 2,
            8 => 3,
            _ => return Err("non-cardinal crossing side".into()),
        };
        let lane = u32::from(record[slot]).wrapping_sub(world.lane_base) as usize;
        if lane >= world.lanes.len() {
            return Err("crossing lane missing".into());
        }
        Ok(Some(lane))
    }

    /// Original0045d5f0/0044c550 permit crossing only when all four lane
    /// occupancy words are zero. Missing junction links permit it immediately.
    pub fn crossing_allowed(&self, world: &CrossingWorld<'_>) -> Result<bool, String> {
        Ok(self
            .crossing_lane(world)?
            .is_none_or(|lane| world.lanes[lane][8..24].iter().all(|&b| b == 0)))
    }

    /// Original0045d6c0/0045d7a0 and crossing counter updates0044c5c0/0044c620.
    pub fn set_crossing_active(
        &mut self,
        world: &mut CrossingWorld<'_>,
        active: bool,
    ) -> Result<(), String> {
        self.bytes[0x7e] = (self.bytes[0x7e] & !2) | if active { 2 } else { 0 };
        let heading = u32::from_le_bytes(self.bytes[0xa8..0xac].try_into().unwrap());
        if !matches!(heading, 1 | 2 | 4 | 8) {
            return Ok(());
        }
        if let Some(lane) = self.crossing_lane(world)? {
            let counter = u32::from_le_bytes(world.lanes[lane][24..28].try_into().unwrap());
            world.lanes[lane][24..28].copy_from_slice(
                &if active {
                    counter.wrapping_add(1)
                } else {
                    counter.wrapping_sub(1)
                }
                .to_le_bytes(),
            );
        }
        Ok(())
    }

    /// Original0045ecb0 normal category traversal. Route lookup supplies the
    /// original next-hop byte; crossing callbacks run before the category commit.
    pub fn advance_category_travel(
        &mut self,
        boundary: i32,
        centers: &[[f32; 2]; 52],
        counts: &mut [u32; 52],
        mut next_hop: impl FnMut(u8, u8) -> Result<u8, String>,
        mut crossing: impl FnMut(CrossingEvent, &mut Self) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let word = |r: &Self, at| u32::from_le_bytes(r.bytes[at..at + 4].try_into().unwrap());
        let put = |r: &mut Self, at, v: u32| r.bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let clear = |r: &mut Self| {
            for at in [0xd8, 0xe0, 0xe4] {
                r.bytes[at..at + 4].fill(0);
            }
        };
        let mut phase = word(self, 0xd4);
        if phase == 0 {
            if self.bytes[0x7e] & 1 != 0 {
                put(self, 0xd4, 4);
                clear(self);
                return Ok(false);
            }
            put(self, 0xd4, 3);
            clear(self);
            phase = 3;
        }
        match phase {
            3 => {
                let current = self.bytes[0x7c];
                let target = self.bytes[0x7d];
                if current == target {
                    return Ok(true);
                }
                let next = next_hop(current, target)?;
                self.bytes[0x8d] = next;
                if next == 0xff {
                    put(self, 0xd0, 4);
                    put(self, 0xd4, 0);
                    clear(self);
                    self.bytes[0x8d] = target;
                    return Ok(false);
                }
                let destination = centers
                    .get(usize::try_from(next as i8).map_err(|_| "negative next category")?)
                    .ok_or("next category outside centers")?;
                let source = centers
                    .get(usize::from(current))
                    .ok_or("current category outside centers")?;
                let mut heading = word(self, 0xa8);
                if destination[0] > source[0] {
                    heading = 2;
                }
                if destination[0] < source[0] || destination[0].is_nan() || source[0].is_nan() {
                    heading = 8;
                }
                if destination[1] > source[1] {
                    heading = 1;
                }
                if destination[1] < source[1] || destination[1].is_nan() || source[1].is_nan() {
                    heading = 4;
                }
                put(self, 0xa8, heading);
                let rank = |v: u32| {
                    HEADING_RANK
                        .get(v as usize)
                        .copied()
                        .ok_or("travel heading outside table")
                };
                let delta = i32::from(rank(word(self, 0x80))?.wrapping_sub(rank(heading)?) as i8);
                let positive = if delta > 0 {
                    delta < 4
                } else {
                    delta.abs() > 4
                };
                let approach = step_heading(heading, positive);
                put(self, 0x88, approach);
                for (at, value) in [0x90, 0x94, 0x98]
                    .into_iter()
                    .zip(heading_position(approach, self.bytes[0x8c] as i8))
                {
                    put(self, at, value.to_bits());
                }
                put(self, 0xd4, 1);
                clear(self);
            }
            4 => {
                self.advance_local_target(counts, |_, r| r.schedule_local_movement(centers))?;
            }
            1 => {
                if self.advance_entrance_approach(boundary, centers, |r| {
                    r.advance_perimeter(centers)
                })? {
                    put(self, 0xd4, 2);
                    clear(self);
                }
            }
            2 => {
                let mut inner = word(self, 0xd8);
                if inner == 0 {
                    crossing(CrossingEvent::Prepare, self)?;
                    put(self, 0xd8, 4);
                    put(self, 0xe0, 0);
                    put(self, 0xe4, 0);
                    inner = 4;
                }
                match inner {
                    4 => {
                        let gate = word(self, 0xe0);
                        if gate != 0 && gate != 4 {
                            return Ok(false);
                        }
                        if gate == 0 {
                            put(self, 0xe0, 4);
                            put(self, 0xe4, 0);
                        }
                        if crossing(CrossingEvent::Gate, self)? {
                            crossing(CrossingEvent::Animation(2), self)?;
                            crossing(CrossingEvent::Begin, self)?;
                            put(self, 0xd8, 2);
                            put(self, 0xe0, 0);
                            put(self, 0xe4, 0);
                        } else {
                            crossing(CrossingEvent::WaitingAnimation, self)?;
                        }
                    }
                    2 => {
                        self.schedule_local_movement(centers)?;
                        put(self, 0xd8, 5);
                        put(self, 0xe0, 0);
                        put(self, 0xe4, 0);
                    }
                    5 => {
                        crossing(CrossingEvent::Finish, self)?;
                        put(self, 0x80, word(self, 0x84));
                        let old = usize::from(self.bytes[0x7c]);
                        let next = usize::from(self.bytes[0x8d]);
                        if old >= 52 || next >= 52 {
                            return Err("crossing category outside table".into());
                        }
                        counts[old] = counts[old].wrapping_sub(1);
                        self.bytes[0x7c] = next as u8;
                        counts[next] = counts[next].wrapping_add(1);
                        self.bytes[0x2c + next / 4] |= 3 << ((next % 4) * 2);
                        for (at, value) in [0xac, 0xb0, 0xb4]
                            .into_iter()
                            .zip(heading_position(word(self, 0x80), self.bytes[0x8c] as i8))
                        {
                            put(self, at, value.to_bits());
                        }
                        put(self, 0xd4, 3);
                        clear(self);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(false)
    }

    /// Tick through local travel and purchase with all movement/transaction
    /// dependencies connected. Other purchase stages remain caller-owned.
    pub fn tick_with_local_purchase(
        &mut self,
        auxiliary: &mut [u8; 284],
        state: &mut LocalPurchaseState<'_, '_>,
        mut tick_helper: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut other: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick_with_purchase_actions(
            auxiliary,
            state.centers,
            |event, r, a| {
                if event == TickEvent::Movement {
                    Ok(r.advance_movement(a))
                } else {
                    tick_helper(event, r, a)
                }
            },
            |helper, r, a| match helper {
                ActionHelper::At45e680 => r.advance_local_destination(
                    state.boundary,
                    state.centers,
                    state.category_counts,
                ),
                ActionHelper::At45f660 => {
                    r.purchase_selected_with_postings(
                        state.retail,
                        state.businesses,
                        state.accounts,
                        state.owners,
                        state.postings,
                    )?;
                    Ok(false)
                }
                _ => other(helper, r, a),
            },
        )
    }

    /// Original0045e430 local roaming. Animation setup and perimeter traversal
    /// run synchronously; the caller ignores this original routine's AL value.
    pub fn advance_local_roaming(
        &mut self,
        centers: &[[f32; 2]; 52],
        category_counts: &mut [u32; 52],
        mut animation: impl FnMut(&mut Self) -> Result<(), String>,
        mut perimeter: impl FnMut(&mut Self) -> Result<bool, String>,
    ) -> Result<(), String> {
        let word = |r: &Self, at| u32::from_le_bytes(r.bytes[at..at + 4].try_into().unwrap());
        let put = |r: &mut Self, at, value: u32| {
            r.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes())
        };
        let clear = |r: &mut Self| {
            for at in [0xd8, 0xe0, 0xe4] {
                r.bytes[at..at + 4].fill(0);
            }
        };
        match word(self, 0xd4) {
            0 => {
                animation(self)?;
                if self.bytes[0x7e] & 1 != 0 {
                    put(self, 0xd4, 4);
                    clear(self);
                    return Ok(());
                }
                put(self, 0xd4, 1);
                clear(self);
                let heading = word(self, 0x80);
                put(self, 0x88, step_heading(heading, self.bytes[0x7e] & 8 != 0));
                if perimeter(self)? {
                    put(self, 0xd4, 0);
                    clear(self);
                }
            }
            1 => {
                if perimeter(self)? {
                    put(self, 0xd4, 0);
                    clear(self);
                }
            }
            4 => {
                let inner = word(self, 0xd8);
                if !matches!(inner, 0 | 2 | 3) {
                    return Ok(());
                }
                let category = self.bytes[0x7c] as usize;
                if category >= 52 {
                    return Err("roaming category outside table".into());
                }
                if inner == 0 {
                    self.bytes[0x7e] &= !1;
                    category_counts[category] = category_counts[category].wrapping_add(1);
                    let x = f32::from_bits(word(self, 0x9c));
                    let z = f32::from_bits(word(self, 0xa4));
                    // FCOM unordered follows the native carry-bit branch.
                    let (at, value) = if z.abs() < x.abs() || x.is_nan() || z.is_nan() {
                        (0x9c, x)
                    } else {
                        (0xa4, z)
                    };
                    put(
                        self,
                        at,
                        if value < 0. || value.is_nan() {
                            (-37_f32).to_bits()
                        } else {
                            37_f32.to_bits()
                        },
                    );
                    put(self, 0xd8, 2);
                    put(self, 0xe0, 0);
                    put(self, 0xe4, 0);
                }
                // This inline native sequence is identical to0045d560.
                self.schedule_local_movement(centers)?;
                put(self, 0xd4, 0);
                clear(self);
            }
            _ => {}
        }
        Ok(())
    }

    /// Original0045d900: schedule successive perimeter headings until the
    /// chosen approach heading is reached. Movement itself runs in the tick.
    pub fn advance_perimeter(&mut self, centers: &[[f32; 2]; 52]) -> Result<bool, String> {
        let word = |r: &Self, at| u32::from_le_bytes(r.bytes[at..at + 4].try_into().unwrap());
        let put = |r: &mut Self, at, v: u32| r.bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let state = word(self, 0xd8);
        if state == 0 {
            let current = word(self, 0x80);
            let target = word(self, 0x88);
            if current == target {
                return Ok(true);
            }
            let current_rank = *HEADING_RANK
                .get(current as usize)
                .ok_or("perimeter heading outside table")?;
            let target_rank = *HEADING_RANK
                .get(target as usize)
                .ok_or("perimeter heading outside table")?;
            let delta = i32::from(current_rank.wrapping_sub(target_rank) as i8);
            let positive = if delta > 0 {
                delta >= 4
            } else {
                delta.abs() <= 4
            };
            self.bytes[0x7e] = (self.bytes[0x7e] & !8) | if positive { 8 } else { 0 };
            let next = step_heading(current, positive);
            put(self, 0x84, next);
            for (at, value) in [0x9c, 0xa0, 0xa4]
                .into_iter()
                .zip(heading_position(next, self.bytes[0x8c] as i8))
            {
                put(self, at, value.to_bits());
            }
            put(self, 0xd8, 2);
            put(self, 0xe0, 0);
            put(self, 0xe4, 0);
        } else if state != 2 {
            return Ok(false);
        }
        self.schedule_local_movement(centers)?;
        let next = word(self, 0x84);
        put(self, 0x80, next);
        if next == word(self, 0x88) {
            return Ok(true);
        }
        for at in [0xd8, 0xe0, 0xe4] {
            put(self, at, 0);
        }
        Ok(false)
    }

    /// Complete local target path, including perimeter approach and scheduling.
    pub fn advance_local_destination(
        &mut self,
        boundary: i32,
        centers: &[[f32; 2]; 52],
        category_counts: &mut [u32; 52],
    ) -> Result<bool, String> {
        self.advance_local_target(category_counts, |helper, r| match helper {
            ActionHelper::At45d560 => r.schedule_local_movement(centers),
            ActionHelper::At45e080 => {
                r.advance_entrance_approach(boundary, centers, |r| r.advance_perimeter(centers))
            }
            _ => unreachable!(),
        })
    }

    /// Original0045e080: choose a perimeter approach, traverse it, then
    /// schedule the edge and final entrance legs. The perimeter helper may
    /// mutate the nested state; preserve its writes before restoring d8/dc.
    pub fn advance_entrance_approach(
        &mut self,
        boundary: i32,
        centers: &[[f32; 2]; 52],
        mut perimeter: impl FnMut(&mut Self) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let word = |r: &Self, at| u32::from_le_bytes(r.bytes[at..at + 4].try_into().unwrap());
        let put = |r: &mut Self, at, v: u32| r.bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let clamp = |mut v: [f32; 3]| {
            let at = if v[2].abs() < v[0].abs() || v[0].is_nan() || v[2].is_nan() {
                0
            } else {
                2
            };
            v[at] = if v[at] < 0. || v[at].is_nan() {
                -37.
            } else {
                37.
            };
            v
        };
        match word(self, 0xd8) {
            0 => {
                let target = clamp([0x90, 0x94, 0x98].map(|at| f32::from_bits(word(self, at))));
                let current = [0xac, 0xb0, 0xb4].map(|at| f32::from_bits(word(self, at)));
                let target = position_headings(target, boundary);
                let current = position_headings(current, boundary);
                let distance = |a: u32, b: u32| {
                    let d = (i32::from(HEADING_RANK[a as usize])
                        - i32::from(HEADING_RANK[b as usize]))
                    .abs();
                    (if d > 4 { 8 - d } else { d }) as i8
                };
                let start = if distance(target[0], current[0]) > distance(target[0], current[1]) {
                    current[0]
                } else {
                    current[1]
                };
                let finish = if distance(start, target[0]) > distance(start, target[1]) {
                    target[1]
                } else {
                    target[0]
                };
                put(self, 0x80, start);
                put(self, 0x88, finish);
                for at in [0xe0, 0xe4, 0xdc] {
                    put(self, at, 0);
                }
                put(self, 0xd8, 1);
            }
            1 => {
                self.bytes[0x7e] |= 0x10;
                put(self, 0xd8, word(self, 0xdc));
                put(self, 0xdc, 1);
                let done = perimeter(self)?;
                self.bytes[0x7e] &= !0x10;
                let inner = word(self, 0xd8);
                put(self, 0xd8, word(self, 0xdc));
                put(self, 0xdc, inner);
                if done {
                    let target = clamp([0x90, 0x94, 0x98].map(|at| f32::from_bits(word(self, at))));
                    for (at, value) in [0x9c, 0xa0, 0xa4].into_iter().zip(target) {
                        put(self, at, value.to_bits());
                    }
                    put(self, 0xd8, 2);
                    put(self, 0xe0, 0);
                    put(self, 0xe4, 0);
                }
            }
            2 => {
                self.schedule_local_movement(centers)?;
                put(self, 0xe0, 0);
                put(self, 0xe4, 0);
                put(self, 0xd8, 3);
                self.bytes.copy_within(0x90..0x9c, 0x9c);
            }
            3 => return self.schedule_local_movement(centers),
            _ => {}
        }
        Ok(false)
    }

    /// Original0045b770/0045b850: advance the embedded movement state and
    /// publish position/yaw. Completion is reported on the following call;
    /// the original does not snap a short remaining segment to its target.
    pub fn advance_movement(&mut self, auxiliary: &mut [u8; 284]) -> bool {
        let read =
            |bytes: &[u8], at| f64::from(f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()));
        let write = |bytes: &mut [u8], at, value: f64| {
            bytes[at..at + 4].copy_from_slice(&(value as f32).to_le_bytes());
        };
        let mut done = false;
        if self.bytes[0x10c] != 0 {
            self.bytes[0x10c] = 0;
            let same = |a: f64, b: f64| a == b || a.is_nan() || b.is_nan();
            if same(read(&self.bytes, 0x124), read(&self.bytes, 0x130))
                && same(read(&self.bytes, 0x12c), read(&self.bytes, 0x138))
            {
                self.bytes[0x10c] = 1;
                done = true;
            } else {
                let dx = read(&self.bytes, 0x130) - read(&self.bytes, 0x124);
                let dz = (read(&self.bytes, 0x138) - read(&self.bytes, 0x12c)) as f32;
                write(&mut self.bytes, 0x13c, dx);
                write(&mut self.bytes, 0x140, 0.);
                write(&mut self.bytes, 0x144, f64::from(dz));
                let x = dx + read(&self.bytes, 0x124);
                let z = f64::from(dz) + read(&self.bytes, 0x12c);
                write(&mut self.bytes, 0x124, x);
                write(&mut self.bytes, 0x12c, z);
                self.bytes.copy_within(0x124..0x130, 0x100);
                let vx = (read(&self.bytes, 0x100) - read(&self.bytes, 0xf4)) as f32;
                let vz = (read(&self.bytes, 0x108) - read(&self.bytes, 0xfc)) as f32;
                let length = (f64::from(vx) * f64::from(vx) + f64::from(vz) * f64::from(vz)).sqrt();
                let inverse = 1. / length;
                write(&mut self.bytes, 0x110, inverse * f64::from(vx));
                write(&mut self.bytes, 0x114, inverse * 0.);
                write(&mut self.bytes, 0x118, inverse * f64::from(vz));
                write(&mut self.bytes, 0x11c, length);
            }
        }
        if !done {
            let remaining = read(&self.bytes, 0x11c);
            let speed = read(&self.bytes, 0x120);
            if remaining > speed {
                let x = read(&self.bytes, 0x110) * speed + read(&self.bytes, 0xf4);
                let z = read(&self.bytes, 0x118) * speed + read(&self.bytes, 0xfc);
                write(&mut self.bytes, 0xf4, x);
                write(&mut self.bytes, 0xfc, z);
                write(&mut self.bytes, 0x11c, remaining - speed);
            } else {
                self.bytes[0x10c] = 1;
            }
        }
        auxiliary[4..16].copy_from_slice(&self.bytes[0xf4..0x100]);
        let dx_retained = read(&self.bytes, 0x100) - read(&self.bytes, 0xf4);
        let dx = dx_retained as f32;
        let dz = (read(&self.bytes, 0x108) - read(&self.bytes, 0xfc)) as f32;
        if !(dx_retained == 0. || dx_retained.is_nan()) || !(dz == 0. || dz.is_nan()) {
            let yaw = if dz.abs() < dx.abs() || dx.is_nan() || dz.is_nan() {
                if dx > 0. { 90. } else { -90. }
            } else if dz > 0. {
                180.
            } else {
                0.
            };
            write(auxiliary, 0x14, yaw);
        }
        done
    }

    /// Original0045b950 embedded movement target setter. Retain raw targets
    /// alongside the original bit-masked coordinates; do not approximate floor.
    pub fn set_movement_target(&mut self, target: [f32; 2]) {
        for (at, value) in [(0x130, target[0]), (0x134, 0.), (0x138, target[1])] {
            let bits = value.to_bits();
            let shift = (bits.wrapping_sub(0x3b800000) >> 23) & 31;
            self.bytes[at..at + 4]
                .copy_from_slice(&(bits & (i32::MIN >> shift) as u32).to_le_bytes());
        }
        self.bytes[0x100..0x104].copy_from_slice(&target[0].to_le_bytes());
        self.bytes[0x108..0x10c].copy_from_slice(&target[1].to_le_bytes());
    }

    /// Original0045d560 schedules movement to working target9c using native
    /// category/world offsets. Its true return is scheduling success, not arrival;
    /// mode1 and flag04 prevent the next action until movement completes.
    pub fn schedule_local_movement(&mut self, centers: &[[f32; 2]; 52]) -> Result<bool, String> {
        if u32::from_le_bytes(self.bytes[0xe0..0xe4].try_into().unwrap()) == 0 {
            let center = centers
                .get(usize::from(self.bytes[0x7c]))
                .ok_or("movement category missing")?;
            let x = f32::from_le_bytes(self.bytes[0x9c..0xa0].try_into().unwrap());
            let z = f32::from_le_bytes(self.bytes[0xa4..0xa8].try_into().unwrap());
            self.set_movement_target([
                (f64::from(x) + f64::from(center[0]) + 470.) as f32,
                (f64::from(z) + f64::from(center[1]) + 450.) as f32,
            ]);
            self.bytes[0x7e] |= 4;
            self.bytes[0xb8..0xbc].copy_from_slice(&1_u32.to_le_bytes());
        }
        Ok(true)
    }

    /// Resolve0045f660's selected building, catalog slot, spending account,
    /// preference cap and seller before executing the shared buyer/seller path.
    pub fn purchase_selected(
        &mut self,
        world: &RetailWorld<'_>,
        businesses: &mut [[u8; 544]],
        accounts: &mut [SpendingAccount],
        owners: &mut [crate::economy::PlayerAccount],
    ) -> Result<Option<i32>, String> {
        self.purchase_selected_with_postings(world, businesses, accounts, owners, &mut Vec::new())
    }

    /// Retain each actual seller posting, using its independently rounded amount.
    pub fn purchase_selected_with_postings(
        &mut self,
        world: &RetailWorld<'_>,
        businesses: &mut [[u8; 544]],
        accounts: &mut [SpendingAccount],
        owners: &mut [crate::economy::PlayerAccount],
        postings: &mut Vec<crate::economy::AccountPosting>,
    ) -> Result<Option<i32>, String> {
        let building = i16::from_le_bytes(self.bytes[0x46..0x48].try_into().unwrap());
        let link = usize::try_from(building)
            .ok()
            .and_then(|i| world.building_businesses.get(i))
            .ok_or("retail building leaves mapping table")?;
        let Some(index) = *link else {
            return Ok(None);
        };
        let business = businesses
            .get_mut(index)
            .ok_or("retail mapping leaves business pool")?;
        let kind = u32::from_le_bytes(business[4..8].try_into().unwrap()) as usize;
        let catalog = world.catalogs.get(kind).ok_or("retail catalog missing")?;
        let commodity = u32::from_le_bytes(self.bytes[0x64..0x68].try_into().unwrap());
        let Some(slot) = catalog.iter().position(|&id| id == commodity) else {
            return Ok(None);
        };
        if u32::from_le_bytes(business[0x6c..0x70].try_into().unwrap()) != 0 {
            return Ok(None);
        }
        let account = i16::from_le_bytes(self.bytes[0x68..0x6a].try_into().unwrap());
        let account = usize::try_from(account)
            .ok()
            .and_then(|i| accounts.get_mut(i))
            .ok_or("retail account leaves spending pool")?;
        let preferences = world
            .preferences
            .get(self.bytes[0x6a] as i8)
            .ok_or("retail preference row missing")?;
        let preference_limit = preferences.get(slot).map_or(0., |entry| entry.factor);
        let interaction_delay = *world
            .interaction_delays
            .get(kind)
            .ok_or("retail delay missing")?;
        let unit_cost = *world
            .unit_costs
            .get(commodity as usize)
            .ok_or("retail commodity missing")?;
        let owner = if business[0xa] == 0 {
            None
        } else {
            Some(
                owners
                    .get_mut(usize::from(business[0xa] - 1))
                    .ok_or("retail owner leaves player pool")?,
            )
        };
        let owner_id = business[0xa];
        self.purchase_resolved_with_postings(
            business,
            account,
            RetailInputs {
                slot,
                preference_limit,
                interaction_delay,
                unit_cost,
                tick: world.tick,
            },
            owner,
            |amount| {
                postings.push(crate::economy::AccountPosting {
                    player: usize::from(owner_id - 1),
                    amount,
                })
            },
        )
    }

    /// Local target helper0045e680. The two remaining route/motion helpers
    /// execute synchronously; arrival is reported only by the native state5.
    pub fn advance_local_target(
        &mut self,
        category_counts: &mut [u32; 52],
        mut motion: impl FnMut(ActionHelper, &mut Self) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let word = |resident: &Self, at| {
            u32::from_le_bytes(resident.bytes[at..at + 4].try_into().unwrap())
        };
        let reset = |resident: &mut Self, state: u32| {
            resident.bytes[0xd4..0xd8].copy_from_slice(&state.to_le_bytes());
            for at in [0xd8, 0xe0, 0xe4] {
                resident.bytes[at..at + 4].fill(0);
            }
        };
        match word(self, 0xd4) {
            0 => reset(self, if self.bytes[0x7e] & 1 != 0 { 4 } else { 6 }),
            4 => {
                let sub = word(self, 0xd8);
                if sub == 0 {
                    self.bytes[0x7e] &= !1;
                    let count = category_counts
                        .get_mut(usize::from(self.bytes[0x7c]))
                        .ok_or("local target category missing")?;
                    *count = count.wrapping_add(1);
                    let x = f32::from_bits(word(self, 0x9c));
                    let z = f32::from_bits(word(self, 0xa4));
                    // Original unordered comparisons take the X branch and
                    // choose its negative edge, matching x87 C0 behavior.
                    let x_axis = z.abs() < x.abs() || z.is_nan() || x.is_nan();
                    let (at, value) = if x_axis { (0x9c, x) } else { (0xa4, z) };
                    let edge = if value < 0. || value.is_nan() {
                        -37_f32
                    } else {
                        37_f32
                    };
                    self.bytes[at..at + 4].copy_from_slice(&edge.to_le_bytes());
                    self.bytes[0xd8..0xdc].copy_from_slice(&2_u32.to_le_bytes());
                    for at in [0xe0, 0xe4] {
                        self.bytes[at..at + 4].fill(0);
                    }
                }
                if (sub == 0 || sub == 2 || sub == 3) && motion(ActionHelper::At45d560, self)? {
                    reset(self, 0);
                }
            }
            5 => {
                self.bytes[0x54] = 0xff;
                self.bytes[0x7e] |= 1;
                let count = category_counts
                    .get_mut(usize::from(self.bytes[0x7c]))
                    .ok_or("local target category missing")?;
                *count = count.wrapping_sub(1);
                return Ok(true);
            }
            6 if motion(ActionHelper::At45e080, self)? => reset(self, 5),
            _ => {}
        }
        Ok(false)
    }

    /// Resident tick with movement and purchase category/business decisions.
    /// Other actions and transactions retain synchronous callbacks so their
    /// writes are observed in the original order.
    pub fn tick_with_purchase_decisions(
        &mut self,
        auxiliary: &mut [u8; 284],
        inputs: &PurchaseDecisions<'_, '_>,
        owners: &mut [crate::economy::PlayerAccount],
        mut tick_helper: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut remaining: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick_with_actions(
            auxiliary,
            inputs.world.centers,
            |event, resident, auxiliary| {
                if event == TickEvent::Movement {
                    Ok(resident.advance_movement(auxiliary))
                } else {
                    tick_helper(event, resident, auxiliary)
                }
            },
            |helper, resident, auxiliary| {
                if helper == ActionHelper::At45f950 {
                    resident.advance_purchase_decisions(auxiliary, inputs, owners, &mut remaining)
                } else {
                    remaining(helper, resident, auxiliary)
                }
            },
        )
    }

    /// Purchase action0045f950 with its two selection helpers integrated.
    /// Other helpers keep their existing explicit movement/transaction boundary.
    pub fn advance_purchase_decisions(
        &mut self,
        auxiliary: &mut [u8; 284],
        inputs: &PurchaseDecisions<'_, '_>,
        owners: &mut [crate::economy::PlayerAccount],
        mut remaining: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<bool, String> {
        self.advance_purchase_action(auxiliary, |helper, resident, auxiliary| match helper {
            ActionHelper::At45abd0 | ActionHelper::At45ac90 => {
                resident.purchase_decision(helper, auxiliary, inputs, owners)
            }
            _ => remaining(helper, resident, auxiliary),
        })
    }

    fn purchase_decision(
        &mut self,
        helper: ActionHelper,
        auxiliary: &[u8; 284],
        inputs: &PurchaseDecisions<'_, '_>,
        owners: &mut [crate::economy::PlayerAccount],
    ) -> Result<bool, String> {
        let position =
            [4, 12].map(|at| f32::from_le_bytes(auxiliary[at..at + 4].try_into().unwrap()));
        match helper {
            ActionHelper::At45abd0 => self.choose_purchase_category(
                inputs.world,
                inputs.accounts,
                position,
                inputs.category_owners,
                owners,
            ),
            ActionHelper::At45ac90 => {
                // Native reads the account only inside a nonempty preference loop.
                let mask = if inputs.preferences.is_empty() {
                    0
                } else {
                    let index = i16::from_le_bytes(self.bytes[0x68..0x6a].try_into().unwrap());
                    let account = usize::try_from(index)
                        .ok()
                        .and_then(|i| inputs.accounts.get(i))
                        .ok_or("purchase decision account leaves spending pool")?;
                    u32::from_le_bytes(account.bytes[0x84..0x88].try_into().unwrap())
                };
                self.choose_purchase_business(
                    inputs.world,
                    inputs.accounts,
                    PurchaseBusinessInputs {
                        preferences: inputs.preferences,
                        account_mask: mask,
                        nodes: inputs.nodes,
                        auxiliary_position: position,
                        distance_weight: inputs.business_distance_weight,
                    },
                    inputs.entrances,
                )
            }
            _ => Err("not a purchase selection helper".into()),
        }
    }

    /// Resolve candidate scores through the same business data as category
    /// choice, then complete the selected entrance. Nodes correspond exactly
    /// to the current category's chain, including unresolved entries.
    pub fn choose_purchase_business(
        &mut self,
        world: &PurchaseWorld<'_>,
        accounts: &[SpendingAccount],
        inputs: PurchaseBusinessInputs<'_>,
        entrances: &[[f32; 3]],
    ) -> Result<bool, String> {
        let category = usize::from(self.bytes[0x7c]);
        let chain = world
            .chains
            .get(category)
            .ok_or("purchase current category missing")?;
        if chain.len() != inputs.nodes.len() {
            return Err("purchase node data differs from category chain".into());
        }
        for (entry, node) in chain.iter().zip(inputs.nodes) {
            let mask = entry
                .map(|index| {
                    let b = world
                        .businesses
                        .get(index)
                        .ok_or("purchase chain leaves business pool")?;
                    Ok::<u32, String>(u32::from_le_bytes(b[0x70..0x74].try_into().unwrap()))
                })
                .transpose()?;
            if mask != node.business_mask {
                return Err("purchase node mask differs from business record".into());
            }
        }
        self.select_purchase_business(
            inputs,
            |resident, index, commodity| {
                let business = &world.businesses[chain[index].unwrap()];
                let commodity =
                    usize::try_from(commodity).map_err(|_| "purchase commodity overflow")?;
                if commodity >= 32 {
                    return Err("purchase commodity exceeds catalog".into());
                }
                resident.score_purchase_business_record(world, accounts, business, commodity)
            },
            |resident| resident.finish_purchase_destination(entrances),
        )
    }

    /// Original0045cd70 for the purchase destination at44. Entrance coordinates
    /// are runtime building224..22f; copy X/Z bits and force target Y to0.5.
    pub fn finish_purchase_destination(&mut self, entrances: &[[f32; 3]]) -> Result<(), String> {
        let building = i16::from_le_bytes(self.bytes[0x46..0x48].try_into().unwrap());
        let position = usize::try_from(building)
            .ok()
            .and_then(|i| entrances.get(i))
            .ok_or("purchase destination building leaves entrance table")?;
        self.bytes[0x48..0x4c].copy_from_slice(&position[0].to_le_bytes());
        self.bytes[0x4c..0x50].copy_from_slice(&0.5_f32.to_le_bytes());
        self.bytes[0x50..0x54].copy_from_slice(&position[2].to_le_bytes());
        Ok(())
    }

    /// Original0045ac90 after preference/account/current-category resolution.
    /// The first commodity with a successful business ends the search. Destination
    /// completion0045cd70 remains explicit and runs after storing building46.
    pub fn select_purchase_business(
        &mut self,
        inputs: PurchaseBusinessInputs<'_>,
        mut score: impl FnMut(&Self, usize, u32) -> Result<f64, String>,
        finish_destination: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<bool, String> {
        let PurchaseBusinessInputs {
            preferences,
            account_mask,
            nodes,
            auxiliary_position,
            distance_weight,
        } = inputs;
        for &commodity in preferences {
            let bit = 1_u32.wrapping_shl(commodity);
            let demand = u32::from_le_bytes(self.bytes[0x6c..0x70].try_into().unwrap());
            if account_mask & demand & bit == 0 {
                continue;
            }
            self.bytes[0x64..0x68].copy_from_slice(&commodity.to_le_bytes());
            let mut best = 0_f32;
            let mut selected = -1_i32;
            for (index, node) in nodes.iter().enumerate() {
                if !node.business_mask.is_some_and(|mask| mask & bit != 0) {
                    continue;
                }
                let mut value = score(self, index, commodity)?;
                // Native distance bonus is conditional on the raw score's
                // difference from the retained best, not simply always added.
                if value != 0. && !value.is_nan() && (best == 0. || value - f64::from(best) < 1.) {
                    let dx = f64::from(auxiliary_position[0]) - f64::from(node.position[0]);
                    let dz = f64::from(auxiliary_position[1]) - f64::from(node.position[1]);
                    value += (100. - (dx * dx + dz * dz).sqrt()) * f64::from(distance_weight);
                }
                if value > f64::from(best) {
                    best = value as f32;
                    selected = node.building;
                }
            }
            if selected != -1 {
                self.bytes[0x46..0x48].copy_from_slice(&(selected as i16).to_le_bytes());
                finish_destination(self)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Compose0045abd0 ->0045a9e0 ->0045a880/0045ab20 with actual price and
    /// category lookups. Host code supplies topology and original settings.
    pub fn choose_purchase_category(
        &mut self,
        world: &PurchaseWorld<'_>,
        accounts: &[SpendingAccount],
        auxiliary_position: [f32; 2],
        category_owners: &[Option<usize>; 52],
        owners: &mut [crate::economy::PlayerAccount],
    ) -> Result<bool, String> {
        self.select_purchase_category(category_owners, owners, |resident, category| {
            let chain = &world.chains[category];
            let masks = chain
                .iter()
                .map(|node| {
                    node.map(|index| {
                        let record = world
                            .businesses
                            .get(index)
                            .ok_or("purchase chain leaves business pool")?;
                        Ok(u32::from_le_bytes(record[0x70..0x74].try_into().unwrap()))
                    })
                    .transpose()
                })
                .collect::<Result<Vec<_>, String>>()?;
            resident.score_purchase_category(
                category,
                accounts,
                world.category_masks[category],
                &masks,
                |resident, candidate, commodity| {
                    let business = &world.businesses[chain[candidate].unwrap()];
                    resident.score_purchase_business_record(world, accounts, business, commodity)
                },
                |resident, category| {
                    let row =
                        u32::from_le_bytes(resident.bytes[0x3c..0x40].try_into().unwrap()) as usize;
                    let weight = world
                        .weights
                        .get(row)
                        .ok_or("purchase distance settings missing")?[3];
                    resident.purchase_distance_adjustment(
                        category,
                        world.centers,
                        auxiliary_position,
                        weight,
                        world.distance_disabled,
                    )
                },
            )
        })
    }

    fn score_purchase_business_record(
        &self,
        world: &PurchaseWorld<'_>,
        accounts: &[SpendingAccount],
        business: &[u8; 544],
        commodity: usize,
    ) -> Result<f64, String> {
        let row = u32::from_le_bytes(self.bytes[0x3c..0x40].try_into().unwrap()) as usize;
        let weights = world
            .weights
            .get(row)
            .ok_or("purchase settings row missing")?;
        let building = i16::from_le_bytes(business[8..10].try_into().unwrap());
        let attribute = usize::try_from(building)
            .ok()
            .and_then(|i| world.building_attributes.get(i))
            .copied()
            .unwrap_or(0);
        let item = world.commodities[commodity];
        self.score_purchase_candidate(
            business,
            accounts,
            PurchaseCandidateInputs {
                commodity,
                value: item.value,
                markup: item.markup,
                building_attribute: attribute,
                weights: [weights[0], weights[1], weights[2]],
            },
            |business, commodity| {
                let kind = u32::from_le_bytes(business[4..8].try_into().unwrap()) as usize;
                let catalog = world
                    .catalogs
                    .get(kind)
                    .ok_or("purchase business catalog missing")?;
                Ok(f64::from(purchase_price(
                    business,
                    catalog,
                    commodity as u32,
                )?))
            },
            |business| {
                let category = purchase_business_category(business, world.building_categories)?;
                if usize::try_from(building)
                    .ok()
                    .and_then(|i| world.building_attributes.get(i))
                    .is_none()
                {
                    return Err("purchase building attribute missing".into());
                }
                usize::try_from(category)
                    .ok()
                    .and_then(|i| world.category_factors.get(i))
                    .copied()
                    .ok_or_else(|| "purchase business category factor missing".into())
            },
        )
        .map(|score| score.value)
    }

    /// Original0045ab20. Source position comes from the auxiliary record only
    /// when resident0e is -1; otherwise resident0c selects a category center.
    pub fn purchase_distance_adjustment(
        &self,
        category: usize,
        centers: &[[f32; 2]; 52],
        auxiliary_position: [f32; 2],
        weight: f32,
        disabled: bool,
    ) -> Result<f64, String> {
        if disabled {
            return Ok(0.);
        }
        let target = centers
            .get(category)
            .ok_or("purchase distance target leaves category table")?;
        let source = if i16::from_le_bytes(self.bytes[0xe..0x10].try_into().unwrap()) == -1 {
            auxiliary_position
        } else {
            let source = self.bytes[0xc] as i8;
            *usize::try_from(source)
                .ok()
                .and_then(|i| centers.get(i))
                .ok_or("purchase distance source leaves category table")?
        };
        let dx = f64::from(source[0]) - f64::from(target[0]);
        let dz = f64::from(source[1]) - f64::from(target[1]);
        Ok((100. - (dx * dx + dz * dz).sqrt() * 0.1) * f64::from(weight))
    }

    /// Original0045a880 with catalog/map/settings resolved by the caller.
    /// Optional outputs distinguish stores already performed from early exits.
    pub fn score_purchase_candidate(
        &self,
        business: &[u8; 0x220],
        accounts: &[SpendingAccount],
        inputs: PurchaseCandidateInputs,
        mut price: impl FnMut(&[u8; 0x220], usize) -> Result<f64, String>,
        category_factor: impl FnOnce(&[u8; 0x220]) -> Result<f32, String>,
    ) -> Result<PurchaseCandidateScore, String> {
        let mut result = PurchaseCandidateScore::default();
        let word = |at| u32::from_le_bytes(business[at..at + 4].try_into().unwrap());
        if word(0x218) == 0 {
            return Ok(result);
        }
        if inputs.commodity >= 32 {
            return Err("purchase commodity exceeds catalog".into());
        }
        let modified = business[0x74] != 0;
        let mode = word(0x78) as i32;
        let multiplier = f64::from(f32::from_bits(word(0x7c)));
        let mut first = price(business, inputs.commodity)?;
        if modified && mode == 3 {
            first *= multiplier;
        }
        let threshold = f64::from(inputs.markup)
            * f64::from(inputs.value)
            * f64::from(f32::from_le_bytes(
                self.bytes[0x70..0x74].try_into().unwrap(),
            ));
        // The first native comparison permits unordered operands.
        if first > threshold {
            return Ok(result);
        }
        let index = i16::from_le_bytes(self.bytes[0x68..0x6a].try_into().unwrap());
        let account = usize::try_from(index)
            .ok()
            .and_then(|index| accounts.get(index))
            .ok_or("purchase candidate account leaves spending pool")?;
        let cell = account.float(4 + inputs.commodity * 4);
        let ratio = f64::from(cell) / price(business, inputs.commodity)? * 80.;
        result.ratio = Some(ratio as f32);
        // Compare the retained value, before the output's f32 rounding.
        if ratio.is_nan() || ratio < 80. || word(0x6c) != 0 {
            return Ok(result);
        }
        let factor = f64::from(category_factor(business)?) * 20.;
        result.category_factor = Some(factor as f32);
        let map_term = f64::from(33 * (i32::from(inputs.building_attribute) + 1))
            * f64::from(inputs.weights[2]);
        let ratio_term = f64::from(result.ratio.unwrap()) * f64::from(inputs.weights[0]);
        let factor_term = factor * f64::from(inputs.weights[1]);
        result.value = ratio_term + (map_term + factor_term);
        if modified && (1..=2).contains(&mode) {
            result.value *= multiplier;
        }
        Ok(result)
    }

    /// Original0045a9e0 aggregation. Candidate masks follow category chain
    /// order; None represents an unresolved node. The two deeper scoring
    /// routines remain explicit callbacks. No game pointers are dereferenced.
    pub fn score_purchase_category(
        &self,
        category: usize,
        accounts: &[SpendingAccount],
        category_mask: u32,
        candidate_masks: &[Option<u32>],
        mut candidate_score: impl FnMut(&Self, usize, usize) -> Result<f64, String>,
        adjustment: impl FnOnce(&Self, usize) -> Result<f64, String>,
    ) -> Result<f64, String> {
        let default = f64::from(f32::from_bits(0xccbebc20));
        if category >= 52 {
            return Err("purchase category exceeds category table".into());
        }
        if (self.bytes[0x2c + category / 4] >> ((category % 4) * 2)) & 3 == 0 {
            return Ok(default);
        }
        let index = i16::from_le_bytes(self.bytes[0x68..0x6a].try_into().unwrap());
        // Native -1 becomes a null account followed by a read at+84.
        // Report malformed state instead of inventing a zero account mask.
        let account = usize::try_from(index)
            .ok()
            .and_then(|index| accounts.get(index))
            .ok_or("purchase score account leaves spending pool")?;
        let mask = u32::from_le_bytes(account.bytes[0x84..0x88].try_into().unwrap())
            & u32::from_le_bytes(self.bytes[0x6c..0x70].try_into().unwrap())
            & category_mask;
        if mask == 0 {
            return Ok(default);
        }
        let mut total = 0_f32;
        for commodity in 0..32 {
            let bit = 1_u32 << commodity;
            let mut best = 0_f32;
            if mask & bit != 0 {
                for (candidate, candidate_mask) in candidate_masks.iter().enumerate() {
                    if candidate_mask.is_some_and(|mask| mask & bit != 0) {
                        let score = crate::economy::ftol_low_word(candidate_score(
                            self, candidate, commodity,
                        )?);
                        if f64::from(score) > f64::from(best) {
                            best = score as f32;
                        }
                    }
                }
            }
            total = (f64::from(total) + f64::from(best)) as f32;
        }
        if total == 0. {
            Ok(default)
        } else {
            Ok(adjustment(self, category)? + f64::from(total))
        }
    }

    /// Original0045abd0. Scores arrive in x87 precision; each new best is
    /// stored as f32 before the next comparison. Equal scores keep the first.
    pub fn select_purchase_category(
        &mut self,
        category_owners: &[Option<usize>; 52],
        owners: &mut [crate::economy::PlayerAccount],
        mut score: impl FnMut(&mut Self, usize) -> Result<f64, String>,
    ) -> Result<bool, String> {
        let mut best = 0_f32;
        let mut selected = None;
        for category in 0..52 {
            let value = score(self, category)?;
            if value > f64::from(best) {
                best = value as f32;
                selected = Some(category);
            }
        }
        let Some(category) = selected else {
            return Ok(false);
        };
        self.bytes[0x44] = category as u8;
        if let Some(owner) = category_owners[category] {
            let owner = owners
                .get_mut(owner)
                .ok_or("purchase category owner leaves player pool")?;
            let increment = match self.bytes[0x74] as i8 {
                0 => 40,
                1 => 20,
                2 => 10,
                _ => 0,
            };
            owner.retail_counters[0] = owner.retail_counters[0].wrapping_add(increment);
        }
        self.bytes[0x74] = self.bytes[0x74].wrapping_add(1);
        Ok(true)
    }

    ///0045f660 after enabled building/map and catalog resolution. Reuses the
    /// original seller implementation before debiting the resident spending row.
    pub fn purchase_resolved(
        &mut self,
        business: &mut [u8; 0x220],
        account: &mut SpendingAccount,
        inputs: RetailInputs,
        owner: Option<&mut crate::economy::PlayerAccount>,
    ) -> Result<Option<i32>, String> {
        self.purchase_resolved_with_postings(business, account, inputs, owner, |_| {})
    }

    fn purchase_resolved_with_postings(
        &mut self,
        business: &mut [u8; 0x220],
        account: &mut SpendingAccount,
        inputs: RetailInputs,
        owner: Option<&mut crate::economy::PlayerAccount>,
        mut posting: impl FnMut(f32),
    ) -> Result<Option<i32>, String> {
        let mut owner = owner;
        self.purchase_with_seller(business, account, inputs, |record, commodity, quantity| {
            if let Some(amount) = crate::economy::purchase_business_record(
                record,
                inputs.slot,
                commodity,
                inputs.unit_cost,
                quantity,
                inputs.tick,
                owner.take(),
            )? {
                posting(amount);
            }
            Ok(())
        })
    }

    fn purchase_with_seller(
        &mut self,
        business: &mut [u8; 0x220],
        account: &mut SpendingAccount,
        inputs: RetailInputs,
        mut seller: impl FnMut(&mut [u8; 0x220], u32, i32) -> Result<(), String>,
    ) -> Result<Option<i32>, String> {
        if u32::from_le_bytes(business[0x6c..0x70].try_into().unwrap()) != 0 {
            return Ok(None);
        }
        let commodity = u32::from_le_bytes(self.bytes[0x64..0x68].try_into().unwrap());
        if inputs.slot >= 6 || commodity >= 32 {
            return Err("resolved retail slot or commodity leaves table".into());
        }
        let price_at = 0x1c4 + inputs.slot * 4;
        let price = f32::from_le_bytes(business[price_at..price_at + 4].try_into().unwrap());
        let cell_at = 4 + commodity as usize * 4;
        let mut quantity =
            crate::economy::ftol_low_word(f64::from(account.float(cell_at)) / f64::from(price));
        let stock_at = 0x80 + inputs.slot * 0x34;
        let stock = u16::from_le_bytes(business[stock_at..stock_at + 2].try_into().unwrap());
        if quantity >= i32::from(stock) {
            quantity = i32::from(stock);
        }
        let cap = f64::from(inputs.preference_limit) * 2.;
        let capped = f64::from(quantity) > cap;
        if capped {
            quantity = crate::economy::ftol_low_word(cap);
        }
        seller(business, commodity, quantity)?;
        // The native routine rereads the price after the seller returns.
        let price = f32::from_le_bytes(business[price_at..price_at + 4].try_into().unwrap());
        let amount = f64::from(quantity) * f64::from(price);
        account.debit_purchase(commodity as usize, amount);
        if capped {
            let remainder = f64::from(account.float(cell_at));
            account.debit_purchase(commodity as usize, remainder);
        }
        self.bytes[0xb8..0xbc].copy_from_slice(&3_u32.to_le_bytes());
        self.bytes.copy_within(0x44..0x54, 0x54);
        self.bytes[0xc4..0xc8].copy_from_slice(
            &crate::economy::ftol_low_word(f64::from(inputs.interaction_delay)).to_le_bytes(),
        );
        Ok(Some(quantity))
    }

    /// Compose tick, outer action and the0045f950 purchase substate helper.
    /// Route selection, movement and transaction bodies remain direct helpers.
    pub fn tick_with_purchase_actions(
        &mut self,
        auxiliary: &mut [u8; 284],
        category_centers: &[[f32; 2]; 52],
        tick_helper: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut action_helper: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick_with_actions(
            auxiliary,
            category_centers,
            tick_helper,
            |helper, resident, auxiliary| {
                if helper == ActionHelper::At45f950 {
                    resident.advance_purchase_action(auxiliary, &mut action_helper)
                } else {
                    action_helper(helper, resident, auxiliary)
                }
            },
        )
    }

    /// Original0045f950: a true result occurs only when initial selection fails.
    /// Transaction return value is ignored before the unconditional state6 write.
    pub fn advance_purchase_action(
        &mut self,
        auxiliary: &mut [u8; 284],
        mut callback: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let sub = u32::from_le_bytes(self.bytes[0xd0..0xd4].try_into().unwrap());
        let next: u32 = match sub {
            0 => {
                if !callback(ActionHelper::At45abd0, self, auxiliary)? {
                    return Ok(true);
                }
                self.bytes[0x7d] = self.bytes[0x44];
                3
            }
            3 | 4 => {
                let helper = if sub == 3 {
                    ActionHelper::At45ecb0
                } else {
                    ActionHelper::At45e830
                };
                if !callback(helper, self, auxiliary)? {
                    return Ok(false);
                }
                6
            }
            5 => {
                self.bytes.copy_within(0x48..0x54, 0x90);
                if !callback(ActionHelper::At45e680, self, auxiliary)? {
                    return Ok(false);
                }
                callback(ActionHelper::At45f660, self, auxiliary)?;
                6
            }
            6 => {
                if callback(ActionHelper::At45ac90, self, auxiliary)? {
                    5
                } else {
                    0
                }
            }
            _ => return Ok(false),
        };
        self.bytes[0xd0..0xd4].copy_from_slice(&next.to_le_bytes());
        for offset in [0xd4, 0xd8, 0xe0, 0xe4] {
            self.bytes[offset..offset + 4].fill(0);
        }
        Ok(false)
    }

    /// Compose00460470 with00460090 at its synchronous Action boundary.
    pub fn tick_with_actions(
        &mut self,
        auxiliary: &mut [u8; 284],
        category_centers: &[[f32; 2]; 52],
        mut tick_helper: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut action_helper: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.tick(auxiliary, category_centers, |event, resident, auxiliary| {
            if event == TickEvent::Action {
                resident.advance_action(auxiliary, &mut action_helper)?;
                Ok(false)
            } else {
                tick_helper(event, resident, auxiliary)
            }
        })
    }

    /// Original0045f2e0 home action, including the original four fallback
    /// destinations when home category is unset. The arrival callback is the
    /// explicit0048ded0(index,5.0) boundary, not a housing assignment.
    pub fn advance_home_action(
        &mut self,
        auxiliary: &mut [u8; 284],
        rng: &mut [u32; 3],
        mut action: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
        mut arrival: impl FnMut(u32, f32) -> Result<(), String>,
    ) -> Result<bool, String> {
        let state = u32::from_le_bytes(self.bytes[0xd0..0xd4].try_into().unwrap());
        let next = match state {
            0 => {
                if self.bytes[0xc] == 0xff {
                    rng[0] = rng[0].wrapping_mul(rng[1]).wrapping_add(rng[2]);
                    self.bytes[0xc] = [4, 17, 43, 44][((rng[0] >> 16) & 3) as usize];
                    self.bytes[0x10..0x14].fill(0);
                    self.bytes[0x14..0x18].copy_from_slice(&0.5_f32.to_le_bytes());
                    self.bytes[0x18..0x1c].fill(0);
                }
                self.bytes[0x7d] = self.bytes[0xc];
                Some(3_u32)
            }
            3 | 4 => {
                let helper = if state == 3 {
                    ActionHelper::At45ecb0
                } else {
                    ActionHelper::At45e830
                };
                action(helper, self, auxiliary)?.then_some(5)
            }
            5 => {
                self.bytes.copy_within(0x10..0x1c, 0x90);
                if !action(ActionHelper::At45e680, self, auxiliary)? {
                    return Ok(false);
                }
                if let Some(index) = [4, 17, 43, 44]
                    .iter()
                    .position(|&category| category == self.bytes[0xc])
                {
                    arrival(index as u32, 5.)?;
                }
                return Ok(true);
            }
            _ => None,
        };
        if let Some(next) = next {
            self.bytes[0xd0..0xd4].copy_from_slice(&next.to_le_bytes());
            for at in [0xd4, 0xd8, 0xe0, 0xe4] {
                self.bytes[at..at + 4].fill(0);
            }
        }
        Ok(false)
    }

    /// Original0045f1c0, action12: choose an enabled category and roam there.
    /// The constructor RNG node6fcd40 is shared with creation/countdown work;
    /// callers must retain its state rather than reseeding for each resident.
    pub fn advance_roaming_action(
        &mut self,
        auxiliary: &mut [u8; 284],
        category_enabled: &[u8; 52],
        rng: &mut [u32; 3],
        mut callback: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let state = u32::from_le_bytes(self.bytes[0xd0..0xd4].try_into().unwrap());
        let next = match state {
            0 => {
                // Original loops forever if there is no enabled category.
                if !category_enabled.iter().any(|&enabled| enabled != 0) {
                    return Err("roaming requires an enabled category".into());
                }
                self.bytes[0x7d] = 0xff;
                loop {
                    rng[0] = rng[0].wrapping_mul(rng[1]).wrapping_add(rng[2]);
                    let category = ((rng[0] >> 16) & 0x7fff) as usize % 52;
                    self.bytes[0x7d] = category as u8;
                    if category_enabled[category] != 0 {
                        break;
                    }
                    self.bytes[0x7d] = 0xff;
                }
                Some(3_u32)
            }
            3 | 4 => {
                let helper = if state == 3 {
                    ActionHelper::At45ecb0
                } else {
                    ActionHelper::At45e830
                };
                callback(helper, self, auxiliary)?.then_some(2)
            }
            2 => {
                callback(ActionHelper::At45e430, self, auxiliary)?;
                None
            }
            _ => None,
        };
        if let Some(next) = next {
            self.bytes[0xd0..0xd4].copy_from_slice(&next.to_le_bytes());
            for offset in [0xd4, 0xd8, 0xe0, 0xe4] {
                self.bytes[offset..offset + 4].fill(0);
            }
        }
        Ok(false)
    }

    /// Original0045fa40: choose a housing destination, finish travel, then
    /// attempt assignment on a subsequent update. The assignment callback owns
    /// the linked-list/address/objective changes from0045fab9.
    pub fn advance_housing_action(
        &mut self,
        mut callback: impl FnMut(HousingAction, &mut Self) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let Some(action) = self.housing_action_step() else {
            return Ok(false);
        };
        let success = callback(action, self)?;
        Ok(self.finish_housing_action(action, success))
    }

    pub(crate) fn housing_action_step(&self) -> Option<HousingAction> {
        match u32::from_le_bytes(self.bytes[0xd0..0xd4].try_into().unwrap()) {
            0 => Some(HousingAction::Choose),
            3 => Some(HousingAction::TravelA),
            4 => Some(HousingAction::TravelB),
            7 => Some(HousingAction::Assign),
            _ => None,
        }
    }

    pub(crate) fn finish_housing_action(&mut self, action: HousingAction, success: bool) -> bool {
        if action == HousingAction::Assign && success {
            return true;
        }
        if success {
            self.bytes[0xd0..0xd4].copy_from_slice(&7_u32.to_le_bytes());
            for offset in [0xd4, 0xd8, 0xe0, 0xe4] {
                self.bytes[offset..offset + 4].fill(0);
            }
        } else if matches!(action, HousingAction::Choose | HousingAction::Assign)
            && u32::from_le_bytes(self.bytes[0xcc..0xd0].try_into().unwrap()) != 12
        {
            self.bytes[0xcc..0xd0].copy_from_slice(&12_u32.to_le_bytes());
            for offset in [0xd0, 0xd4, 0xd8, 0xe0, 0xe4] {
                self.bytes[offset..offset + 4].fill(0);
            }
            self.bytes[0xb8..0xbc].copy_from_slice(&1_u32.to_le_bytes());
        }
        false
    }

    /// Original00460090. Deeper helpers run synchronously; destination-state
    /// comparisons deliberately reread the record after each helper returns.
    pub fn advance_action(
        &mut self,
        auxiliary: &mut [u8; 284],
        mut callback: impl FnMut(ActionHelper, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        let state = u32::from_le_bytes(self.bytes[0xcc..0xd0].try_into().unwrap());
        let mut transition = None;
        match state {
            0 => {
                if callback(ActionHelper::At45fa40, self, auxiliary)? {
                    transition = Some((1, 1));
                }
            }
            1 => {
                if callback(ActionHelper::At45f2e0, self, auxiliary)? {
                    transition = Some((2, 4));
                }
            }
            2 | 3 | 5 => {}
            4 => {
                let sub = u32::from_le_bytes(self.bytes[0xd0..0xd4].try_into().unwrap());
                let next = match sub {
                    0 => {
                        self.bytes[0x7d] = self.bytes[0x1c];
                        Some(3_u32)
                    }
                    3 | 4 => {
                        let helper = if sub == 3 {
                            ActionHelper::At45ecb0
                        } else {
                            ActionHelper::At45e830
                        };
                        if callback(helper, self, auxiliary)? {
                            Some(5)
                        } else {
                            None
                        }
                    }
                    5 => {
                        self.bytes.copy_within(0x20..0x2c, 0x90);
                        if callback(ActionHelper::At45e680, self, auxiliary)? {
                            transition = Some((5, 4));
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(next) = next {
                    self.bytes[0xd0..0xd4].copy_from_slice(&next.to_le_bytes());
                    for offset in [0xd4, 0xd8, 0xe0, 0xe4] {
                        self.bytes[offset..offset + 4].fill(0);
                    }
                }
            }
            6..=10 => {
                if callback(ActionHelper::At45f950, self, auxiliary)? {
                    transition = Some((if state == 10 { 1 } else { 12 }, 1));
                }
            }
            12 => {
                callback(ActionHelper::At45f1c0, self, auxiliary)?;
            }
            _ => self.bytes[0xc0..0xc4].copy_from_slice(&625_u32.to_le_bytes()),
        }
        self.finish_action_transition(transition, auxiliary);
        Ok(())
    }

    pub(crate) fn finish_action_transition(
        &mut self,
        transition: Option<(u32, u32)>,
        auxiliary: &mut [u8; 284],
    ) {
        if let Some((next, mode)) = transition {
            let current = u32::from_le_bytes(self.bytes[0xcc..0xd0].try_into().unwrap());
            if current != next {
                self.bytes[0xcc..0xd0].copy_from_slice(&next.to_le_bytes());
                for offset in [0xd0, 0xd4, 0xd8, 0xe0, 0xe4] {
                    self.bytes[offset..offset + 4].fill(0);
                }
                self.bytes[0xb8..0xbc].copy_from_slice(&u32::to_le_bytes(mode));
            }
        }
        auxiliary[0xb8] = (auxiliary[0xb8] & !1) | (!self.bytes[0x7e] & 1);
    }

    /// Original load fixup0045bf31..0045bfa3 after allocation and saved-byte copy.
    /// The auxiliary/model must already be allocated; its reference is opaque.
    /// This advances movement once even when the saved moving flag is clear.
    pub fn restore_saved_movement(
        &mut self,
        auxiliary_reference: u32,
        auxiliary: &mut [u8; 284],
        category_centers: &[[f32; 2]; 52],
    ) -> Result<(), String> {
        self.bytes[0xf0..0xf4].copy_from_slice(&auxiliary_reference.to_le_bytes());
        let done = self.advance_movement(auxiliary);
        self.finish_movement_update(auxiliary, category_centers, done)?;
        auxiliary[0xb8] = (auxiliary[0xb8] & !1) | (!self.bytes[0x7e] & 1);
        Ok(())
    }

    pub(crate) fn finish_movement_update(
        &mut self,
        auxiliary: &mut [u8; 284],
        category_centers: &[[f32; 2]; 52],
        done: bool,
    ) -> Result<(), String> {
        self.bytes[0x7e] = (self.bytes[0x7e] & !4) | if done { 0 } else { 4 };
        for (offset, translation) in [(4, 470.), (12, 450.)] {
            let value = f32::from_le_bytes(auxiliary[offset..offset + 4].try_into().unwrap());
            auxiliary[offset..offset + 4]
                .copy_from_slice(&((f64::from(value) - translation) as f32).to_le_bytes());
        }
        auxiliary[8..12].copy_from_slice(&0.5_f32.to_le_bytes());
        self.bytes[0xac..0xb8].copy_from_slice(&auxiliary[4..16]);
        let center = category_centers
            .get(self.bytes[0x7c] as usize)
            .ok_or("resident category leaves center table")?;
        for (offset, origin) in [(0xac, center[0]), (0xb4, center[1])] {
            let value = f32::from_le_bytes(self.bytes[offset..offset + 4].try_into().unwrap());
            self.bytes[offset..offset + 4]
                .copy_from_slice(&((f64::from(value) - f64::from(origin)) as f32).to_le_bytes());
        }
        Ok(())
    }

    /// Per-tick resident dispatch. Callbacks own the deeper helpers and may
    /// mutate resident/auxiliary state before the subsequent original reads.
    pub fn tick(
        &mut self,
        auxiliary: &mut [u8; 284],
        category_centers: &[[f32; 2]; 52],
        mut callback: impl FnMut(TickEvent, &mut Self, &mut [u8; 284]) -> Result<bool, String>,
    ) -> Result<(), String> {
        if self.bytes[0x7e] & 4 != 0 {
            let done = callback(TickEvent::Movement, self, auxiliary)?;
            self.finish_movement_update(auxiliary, category_centers, done)?;
        }
        if self.advance_tick_countdown() {
            callback(TickEvent::RefreshIntent, self, auxiliary)?;
            self.finish_intent_refresh();
        }
        if self.take_tick_action() {
            callback(TickEvent::Action, self, auxiliary)?;
        }
        Ok(())
    }

    pub(crate) fn advance_tick_countdown(&mut self) -> bool {
        let countdown =
            u32::from_le_bytes(self.bytes[0xc0..0xc4].try_into().unwrap()).wrapping_sub(1);
        self.bytes[0xc0..0xc4].copy_from_slice(&countdown.to_le_bytes());
        countdown as i32 <= 0 && self.bytes[0x7e] & 2 == 0
    }

    pub(crate) fn finish_intent_refresh(&mut self) {
        self.bytes[0xc0..0xc4].copy_from_slice(&625_u32.to_le_bytes());
    }

    pub(crate) fn take_tick_action(&mut self) -> bool {
        let mode = u32::from_le_bytes(self.bytes[0xb8..0xbc].try_into().unwrap());
        match mode {
            0 => self.bytes[0xbc] != 0,
            1 => self.bytes[0x7e] & 4 == 0,
            2 => {
                let ready = self.bytes[0xbd] != 0;
                if ready {
                    self.bytes[0xbd] = 0;
                }
                ready
            }
            3 => {
                let old = u32::from_le_bytes(self.bytes[0xc4..0xc8].try_into().unwrap());
                self.bytes[0xc4..0xc8].copy_from_slice(&old.wrapping_sub(1).to_le_bytes());
                if old as i32 <= 0 {
                    self.bytes[0xb8..0xbc].copy_from_slice(&1_u32.to_le_bytes());
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Original004603e0 model table variant, before the caller's16-byte stride.
    /// Tourists use adult models; pool low bits select the remaining variants.
    pub fn model_variant(&self) -> u8 {
        let age = if self.age() == 4 { 2 } else { self.age() as u8 };
        let index = self.pool_index() as u8;
        let group = (((!index >> 2) & 1) << 2).wrapping_add(age);
        (group << 2).wrapping_add(index & 3)
    }

    /// Complete0045c750 with an already-resolved identity and allocated
    /// spending account. Runtime reset/placement precedes this call. Identity
    /// selection, pool ownership and account allocation remain caller-owned.
    pub fn initialize_personal(
        &mut self,
        identity: [u32; 3],
        account: &mut SpendingAccount,
        inputs: &InitializationInputs<'_>,
        rng: &mut InitializationRng,
    ) -> Result<(), String> {
        self.initialize_personal_state(identity, inputs, rng)?;
        account.allocate(
            inputs.preferences.get(identity[2] as i8).unwrap(),
            inputs.commodities,
            &mut rng.constructor,
        )
    }

    pub(crate) fn initialize_personal_state(
        &mut self,
        identity: [u32; 3],
        inputs: &InitializationInputs<'_>,
        rng: &mut InitializationRng,
    ) -> Result<(), String> {
        let [age, class, preference] = identity;
        if age > 4 || class > 2 || preference > 127 || inputs.hour >= 24 {
            return Err("invalid resolved resident initialization inputs".into());
        }
        inputs
            .preferences
            .get(preference as i8)
            .ok_or("missing resident initialization preference")?;
        self.initialize_personal_fields(age, class, &mut rng.constructor)?;
        rng.movement = rng.movement.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        let remainder = ((rng.movement >> 16) & 0x7fff) % 101;
        //0045b720 retains each product in53-bit precision before its f32 store.
        let speed = ((f64::from(remainder) * f64::from(f32::from_bits(0x3c23d70a)))
            * f64::from(inputs.movement.variation)
            + f64::from(inputs.movement.speed_by_age[age as usize])) as f32;
        self.bytes[0x120..0x124].copy_from_slice(&speed.to_le_bytes());
        self.bytes[0x6a] = preference as u8;
        self.bytes[0x2c..0x3a].fill(0);
        self.refresh_daily_objective(inputs.objectives, &mut rng.objective);
        self.refresh_intent(inputs.timetables, inputs.hour, inputs.commodity_masks)?;
        Ok(())
    }

    /// Initial0045c750 field stores, through0045c903 before movement-speed,
    /// preference, objective and spending-account initialization. Age/class
    /// sentinels must already have been resolved by PreferenceGroups.
    pub fn initialize_personal_fields(
        &mut self,
        age: u32,
        class: u32,
        rng: &mut u32,
    ) -> Result<(), String> {
        if age > 4 || class > 2 {
            return Err("resident construction requires resolved age and class".into());
        }
        let mut draw = || {
            *rng = rng.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            (*rng >> 16) & 0x7fff
        };
        self.bytes[8..12].copy_from_slice(&(draw() % 505 + 96).to_le_bytes());
        let position = if self.bytes[0x148] & 4 == 0 {
            draw() % 127 + 731
        } else {
            draw() % 126 + 603
        };
        self.bytes[4..8].copy_from_slice(&position.to_le_bytes());
        self.bytes[0x3c..0x40].copy_from_slice(&class.to_le_bytes());
        // Each16-byte block preserves its second byte. Do not clear the
        // entire record: the native initializers use byte/word stores.
        for offset in [0x0c, 0x1c, 0x54, 0x44] {
            self.bytes[offset] = 0xff;
            self.bytes[offset + 2..offset + 4].fill(0xff);
            self.bytes[offset + 4..offset + 16].fill(0);
        }
        self.bytes[..4].copy_from_slice(&age.to_le_bytes());
        let multiplier =
            (f64::from(draw() % 101) * f64::from(f32::from_bits(0x3c23d70a)) + 1.5) as f32;
        self.bytes[0x70..0x74].copy_from_slice(&multiplier.to_le_bytes());
        if age == 0 {
            self.bytes[0x1c] = 0x29;
            self.bytes[0x24..0x28].copy_from_slice(&0.5_f32.to_le_bytes());
        }
        if age == 4 {
            self.bytes[0x0c] = [4, 17, 43, 44][(draw() & 3) as usize];
            self.bytes[0x14..0x18].copy_from_slice(&0.5_f32.to_le_bytes());
        }
        Ok(())
    }

    /// Reset and initial category placement0045c2c0/0045c3d0. This preserves
    /// original partial writes and initial movement semantics; it is not an
    /// ongoing movement update or the complete identity/account constructor.
    pub fn initialize_runtime(
        &mut self,
        auxiliary_reference: u32,
        auxiliary: &mut [u8; 0x11c],
        rng: &mut u32,
        countdown_period: i32,
        categories: &mut (impl PlacementCategories + ?Sized),
    ) -> Result<(), String> {
        if [43, 44, 17, 4]
            .iter()
            .any(|&i| categories.placement_center(i).is_none())
        {
            return Err("resident placement requires reconstructed category centers".into());
        }
        self.reset_runtime_state(auxiliary_reference, auxiliary, rng, countdown_period)?;
        let mut draw = || {
            *rng = rng.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            (*rng >> 16) & 0x7fff
        };
        let category = [43, 44, 17, 4][(draw() & 3) as usize];
        self.bytes[0x7c] = category as u8;
        self.bytes[0x7d] = category as u8;
        self.bytes[0x8d] = 0xff;
        self.bytes[0x2c + category / 4] |= 3 << ((category % 4) * 2);
        let center = categories.placement_center(category).unwrap();
        categories.increment_resident_count(category);
        let direction: u32 = [1, 9, 8, 12, 4, 6, 2, 3][(draw() & 7) as usize];
        for offset in [0x80, 0x84, 0x88] {
            self.bytes[offset..offset + 4].copy_from_slice(&direction.to_le_bytes());
        }
        let magnitude = f64::from(i32::from(self.bytes[0x8c] as i8) + 35);
        let dx = match direction {
            2 | 3 | 6 => magnitude,
            8 | 9 | 12 => -magnitude,
            _ => 0.,
        };
        let dz = match direction {
            1 | 3 | 9 => magnitude,
            4 | 6 | 12 => -magnitude,
            _ => 0.,
        };
        let x = (dx + f64::from(center[0]) + 470.) as f32;
        let z = (dz + f64::from(center[1]) + 450.) as f32;
        // Original quantizers retain raw inputs as well as masked float bits.
        let quantize = |value: f32| {
            let bits = value.to_bits();
            let shift = (bits.wrapping_sub(0x3b800000) >> 23) & 31;
            (bits & (i32::MIN >> shift) as u32).to_le_bytes()
        };
        for offset in [0xf4, 0x100] {
            self.bytes[offset..offset + 4].copy_from_slice(&x.to_le_bytes());
            self.bytes[offset + 8..offset + 12].copy_from_slice(&z.to_le_bytes());
        }
        for offset in [0x124, 0x130] {
            self.bytes[offset..offset + 4].copy_from_slice(&quantize(x));
            self.bytes[offset + 4..offset + 8].fill(0);
            self.bytes[offset + 8..offset + 12].copy_from_slice(&quantize(z));
        }
        self.bytes[0x7e] &= !0x0c;
        if draw() & 1 == 0 {
            self.bytes[0x7e] |= 8;
        }
        // Equal source/target takes two distinct0045b770 return paths based
        // on the preserved destination flag. Both set it to1, but only the
        // initially-zero path returns0 and sets the resident's bit04.
        if self.bytes[0x10c] == 0 {
            self.bytes[0x7e] |= 4;
        }
        self.bytes[0x10c] = 1;
        let world_x = (f64::from(x) - 470.) as f32;
        let world_z = (f64::from(z) - 450.) as f32;
        auxiliary[4..8].copy_from_slice(&world_x.to_le_bytes());
        auxiliary[8..12].copy_from_slice(&0.5_f32.to_le_bytes());
        auxiliary[12..16].copy_from_slice(&world_z.to_le_bytes());
        self.bytes[0xac..0xb0]
            .copy_from_slice(&((f64::from(world_x) - f64::from(center[0])) as f32).to_le_bytes());
        self.bytes[0xb0..0xb4].copy_from_slice(&0.5_f32.to_le_bytes());
        self.bytes[0xb4..0xb8]
            .copy_from_slice(&((f64::from(world_z) - f64::from(center[1])) as f32).to_le_bytes());
        self.bytes[0x90..0xa8].fill(0);
        self.bytes[0xa8..0xac].copy_from_slice(&1_u32.to_le_bytes());
        Ok(())
    }

    /// Original0045c2c0 and embedded0045b5b0 reset, before category/identity
    /// construction. `auxiliary_reference` is an opaque engine record handle,
    /// never a Rust pointer. The caller owns its matching auxiliary bytes.
    pub fn reset_runtime_state(
        &mut self,
        auxiliary_reference: u32,
        auxiliary: &mut [u8; 0x11c],
        rng: &mut u32,
        countdown_period: i32,
    ) -> Result<(), String> {
        if countdown_period <= 0 {
            return Err("resident countdown period must be positive".into());
        }
        self.reset_runtime_prefix(auxiliary_reference);
        self.bytes[0xc4..0xc8].fill(0);
        auxiliary[0x14..0x18].fill(0);
        self.bytes[0xec..0xf0].fill(0xff);
        self.bytes[0x64..0x68].fill(0xff);
        self.bytes[0x6c..0x70].fill(0);
        self.bytes[0x74] = 0;
        self.bytes[0xc8..0xd0].fill(0xff);
        self.bytes[0xd0..0xec].fill(0);
        self.bytes[0x7e] &= 0xe8;
        *rng = rng.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        self.bytes[0x8c] = (((*rng >> 16) & 0x7fff) % 5 + 1) as u8;
        *rng = rng.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        self.set_housing_countdown(
            ((((*rng >> 16) & 0x7fff) % countdown_period as u32) + 1) as i32,
        );
        Ok(())
    }

    /// Writes preceding the original null-auxiliary fault at0045c300.
    pub(crate) fn reset_runtime_prefix(&mut self, auxiliary_reference: u32) {
        self.auxiliary_reference = auxiliary_reference;
        self.bytes[0xf0..0xf4].copy_from_slice(&auxiliary_reference.to_le_bytes());
        // Embedded0045b5b0 clears only these fields; notably its destination
        // vector and flag bytes at+10..1f retain their previous contents.
        self.bytes[0xf4..0x100].fill(0);
        self.bytes[0x110..0x120].fill(0);
        self.bytes[0x120..0x124].copy_from_slice(&1_f32.to_le_bytes());
        self.bytes[0x124..0x148].fill(0);
        self.bytes[0xb8..0xbc].copy_from_slice(&1_u32.to_le_bytes());
        self.bytes[0xbc] = 0;
        self.bytes[0xbd] = 1;
        self.bytes[0xc0..0xc4].fill(0);
    }

    pub(crate) fn set_account_index(&mut self, index: Option<usize>) {
        self.bytes[0x68..0x6a].copy_from_slice(&index.map_or(-1, |i| i as i16).to_le_bytes());
    }

    /// Original 0045fc90 intent transition, before movement or purchasing.
    /// Commodity masks follow globals 007f4740..50: snack, shopping,
    /// entertainment, meal, stay-over. Their configuration is caller-owned.
    pub fn refresh_intent(
        &mut self,
        timetables: &Timetables,
        hour: usize,
        commodity_masks: [u32; 5],
    ) -> Result<(), String> {
        let objective =
            usize::try_from(self.bytes[0x40] as i8).map_err(|_| "negative resident timetable")?;
        let action = timetables
            .action(objective, hour)
            .ok_or("invalid resident timetable lookup")?;
        let state = i32::from_le_bytes(self.bytes[0xcc..0xd0].try_into().unwrap());
        let demand = match action {
            9 => Some(0),
            12 => Some(1),
            11 => Some(2),
            10 => Some(3),
            13 => Some(4),
            _ => None,
        };
        if let Some(index) = demand {
            self.bytes[0x6c..0x70].copy_from_slice(&commodity_masks[index].to_le_bytes());
        }
        let next = match action {
            0 => match state {
                1 | 3 => None,
                2 => Some(3),
                _ => Some(1),
            },
            1 | 7 | 8 => Some(12),
            2 if !matches!(state, 1..=3) => Some(1),
            3 | 4 | 15 | 16 if !matches!(state, 4 | 5) => Some(4),
            5 if i16::from_le_bytes(self.bytes[0xe..0x10].try_into().unwrap()) == -1 => Some(0),
            9 => Some(8),
            10 => Some(7),
            11 => Some(9),
            12 => Some(6),
            13 => Some(10),
            14 => match state {
                1 | 2 => None,
                3 => Some(2),
                _ => Some(1),
            },
            17 if state != 2 => Some(1),
            _ => None,
        };
        if let Some(next) = next.filter(|&next| next != state) {
            self.bytes[0xcc..0xd0].copy_from_slice(&i32::to_le_bytes(next));
            for offset in [0xd0, 0xd4, 0xd8, 0xe0, 0xe4] {
                self.bytes[offset..offset + 4].fill(0);
            }
            if matches!(next, 6 | 9) {
                self.bytes[0x74] = 0;
            }
            let mode: u32 = if matches!(next, 2 | 3 | 5) { 4 } else { 1 };
            self.bytes[0xb8..0xbc].copy_from_slice(&mode.to_le_bytes());
        }
        Ok(())
    }

    /// Status/counter tail of original hour-5 pass, after spending refresh.
    /// The caller supplies the objective RNG state; loading a save does not
    /// establish that state here.
    pub(crate) fn update_daily_status(&mut self, objectives: &DailyObjectives, rng: &mut u32) {
        self.refresh_daily_objective(objectives, rng);
        // 0045bd80 visits52 two-bit cells, including all four cells in byte38.
        for index in 0..52 {
            let byte = &mut self.bytes[0x2c + index / 4];
            let shift = (index % 4) * 2;
            let value = (*byte >> shift) & 3;
            if value != 0 {
                *byte = (*byte & !(3 << shift)) | ((value - 1) << shift);
            }
        }
    }

    /// Original 0045ffe0 objective refresh, also used after housing changes.
    pub(crate) fn refresh_daily_objective(&mut self, objectives: &DailyObjectives, rng: &mut u32) {
        let unhoused = i16::from_le_bytes(self.bytes[0x0e..0x10].try_into().unwrap()) == -1;
        let selector = if unhoused && self.age() != 4 {
            1
        } else {
            match self.age() {
                0 => 3,
                1 => 4,
                2 => 0,
                3 => 2,
                4 => 5,
                _ => unreachable!("financial update validates resident age"),
            }
        };
        let row = objectives.get(selector).unwrap();
        *rng = rng.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        self.bytes[0x40] = row[(((*rng >> 16) & 0x7fff) as usize) % row.len()] as u8;
    }

    pub fn housing_building(&self) -> i16 {
        i16::from_le_bytes(self.bytes[0xe..0x10].try_into().unwrap())
    }
    pub fn housing_category(&self) -> i8 {
        self.bytes[0xc] as i8
    }
    pub(crate) fn tick_housing_countdown(&mut self) -> bool {
        let next = (self.bytes[0x78] & 15).wrapping_sub(1) & 15;
        self.bytes[0x78] = (self.bytes[0x78] & 0xf0) | next;
        // After the store, the native signed nibble is positive only for1..7.
        !(1..=7).contains(&next)
    }
    pub(crate) fn set_housing_countdown(&mut self, value: i32) {
        self.bytes[0x78] = (self.bytes[0x78] & 0xf0) | ((value as u8) & 15);
    }
    pub(crate) fn prefer_housing_category(&mut self, category: u8) {
        self.bytes[0x7d] = category;
    }
    pub(crate) fn relocate(&mut self, building: i16, category: i8, position: Option<[f32; 3]>) {
        self.bytes[0xe..0x10].copy_from_slice(&building.to_le_bytes());
        self.bytes[0xc] = category as u8;
        if let Some(mut position) = position {
            position[1] = 0.5;
            for (i, value) in position.iter().enumerate() {
                self.bytes[0x10 + i * 4..0x14 + i * 4]
                    .copy_from_slice(&value.to_bits().to_le_bytes());
            }
        }
    }

    pub fn from_bytes(bytes: [u8; 0x14a]) -> Self {
        Self::from_pool_bytes(bytes, 1)
    }
    /// Runtime pool input; saves contain only occupied records and use from_bytes.
    pub fn from_pool_bytes(bytes: [u8; 0x14a], active: u8) -> Self {
        Self::from_runtime_bytes(bytes, active, 0)
    }
    /// Runtime +14c is separate from the saved embedded movement reference +f0.
    /// References are host-neutral handles, never original process pointers.
    pub fn from_runtime_bytes(bytes: [u8; 0x14a], active: u8, auxiliary_reference: u32) -> Self {
        Self {
            bytes,
            active,
            auxiliary_reference,
        }
    }
    pub fn auxiliary_reference(&self) -> u32 {
        self.auxiliary_reference
    }
    pub(crate) fn overwrite_saved_payload(&mut self, bytes: [u8; 0x14a]) {
        self.bytes = bytes;
        self.bytes[0xf0..0xf4].copy_from_slice(&self.auxiliary_reference.to_le_bytes());
    }
    pub fn is_active(&self) -> bool {
        self.active != 0
    }
    pub fn active_byte(&self) -> u8 {
        self.active
    }
    pub(crate) fn reset_pool_slot(&mut self, index: u16) {
        self.bytes[0x148..0x14a].copy_from_slice(&index.to_le_bytes());
        self.active = 0;
    }
    pub(crate) fn mark_allocated(&mut self) {
        self.active = 1;
    }
    pub fn as_bytes(&self) -> &[u8; 0x14a] {
        &self.bytes
    }
    pub fn age(&self) -> u32 {
        u32::from_le_bytes(self.bytes[..4].try_into().unwrap())
    }
    pub fn class(&self) -> u32 {
        u32::from_le_bytes(self.bytes[0x3c..0x40].try_into().unwrap())
    }
    pub fn account_index(&self) -> i16 {
        i16::from_le_bytes(self.bytes[0x68..0x6a].try_into().unwrap())
    }
    pub fn preference_index(&self) -> i8 {
        self.bytes[0x6a] as i8
    }
    pub fn next_in_housing(&self) -> i32 {
        i32::from_le_bytes(self.bytes[0xec..0xf0].try_into().unwrap())
    }
    pub(crate) fn set_next_in_housing(&mut self, next: i32) {
        self.bytes[0xec..0xf0].copy_from_slice(&next.to_le_bytes());
    }
    pub fn pool_index(&self) -> u16 {
        u16::from_le_bytes(self.bytes[0x148..0x14a].try_into().unwrap())
    }
}

/// People.Welfare settings indexed by class then age, as used by 0045f540.
/// The native table has a larger row stride; only these five age cells belong
/// to this setting. Adjacent native fields are not treated as Welfare values.
pub struct Welfare {
    pub amounts: [[f32; 5]; 3],
}

impl Welfare {
    /// Read the literal DEFAULT block without evaluating other script statements.
    /// Enum registrations: ages004dd0d9..004dd135, classes004d9423..004d9457.
    pub fn from_v14_defaults(source: &str) -> Result<Self, String> {
        let lines: Vec<_> = source
            .lines()
            .map(|l| l.split("--").next().unwrap().trim())
            .collect();
        let prefix = "People.Welfare(";
        let first = lines
            .iter()
            .position(|l| l.starts_with(prefix))
            .ok_or("missing Welfare table")?;
        let last = lines.iter().rposition(|l| l.starts_with(prefix)).unwrap();
        let mut amounts = [[0.; 5]; 3];
        let mut seen = [[false; 5]; 3];
        for &line in &lines[first..=last] {
            if line.is_empty() {
                continue;
            }
            let fields: Vec<_> = line
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(");"))
                .ok_or("non-literal statement inside Welfare table")?
                .split(',')
                .map(str::trim)
                .collect();
            if fields.len() != 3 {
                return Err("Welfare requires age, class and amount".into());
            }
            let age = [
                "AGE_CHILD",
                "AGE_TEENAGER",
                "AGE_ADULT",
                "AGE_PENSIONER",
                "AGE_TOURIST",
            ]
            .iter()
            .position(|&name| name == fields[0])
            .ok_or("unknown Welfare age")?;
            let class = ["CLASS_LOW", "CLASS_MEDIUM", "CLASS_HIGH"]
                .iter()
                .position(|&name| name == fields[1])
                .ok_or("unknown Welfare class")?;
            let amount = fields[2]
                .parse::<f32>()
                .map_err(|_| "invalid Welfare amount")?;
            if seen[class][age] || !amount.is_finite() {
                return Err("duplicate or invalid Welfare entry".into());
            }
            seen[class][age] = true;
            amounts[class][age] = amount;
        }
        if seen.iter().flatten().any(|&v| !v) {
            return Err("incomplete Welfare table".into());
        }
        Ok(Self { amounts })
    }

    pub fn amount(&self, age: u32, class: u32) -> Option<f32> {
        self.amounts.get(class as usize)?.get(age as usize).copied()
    }
}

/// Original 0x90-byte account: preserve the unknown leading word and every
/// spending cell, rather than retaining only the housing balance at +0x88.
#[derive(Clone, Debug)]
pub struct SpendingAccount {
    bytes: [u8; 0x90],
}

impl SpendingAccount {
    fn debit_purchase(&mut self, commodity: usize, amount: f64) {
        self.set_float(0x88, (f64::from(self.float(0x88)) - amount) as f32);
        let at = 4 + commodity * 4;
        let cell = (-amount + f64::from(self.float(at))) as f32;
        self.set_float(at, cell);
        self.set_float(0x8c, (-amount + f64::from(self.float(0x8c))) as f32);
        let mut mask = u32::from_le_bytes(self.bytes[0x84..0x88].try_into().unwrap());
        if cell == 0. || cell.is_nan() {
            mask &= !(1 << commodity);
        } else {
            mask |= 1 << commodity;
        }
        self.bytes[0x84..0x88].copy_from_slice(&mask.to_le_bytes());
    }

    pub fn is_active(&self) -> bool {
        self.bytes[0] != 0
    }

    /// Shared row reset in0045b2f0/0045b330. Padding bytes1..3 survive.
    pub(crate) fn reset_slot(&mut self, active: bool) {
        self.bytes[0] = u8::from(active);
        self.bytes[4..].fill(0);
    }

    pub fn from_bytes(bytes: [u8; 0x90]) -> Self {
        Self { bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 0x90] {
        &self.bytes
    }

    fn float(&self, at: usize) -> f32 {
        f32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }

    fn set_float(&mut self, at: usize, value: f32) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn balance(&self) -> f32 {
        self.float(0x88)
    }

    pub fn set_balance(&mut self, value: f32) {
        self.set_float(0x88, value);
    }

    /// 0045b120. Reconcile the balance at +88 with spending assigned at +8c.
    /// Deficits remove funds in reverse preference order, including inactive
    /// commodities; surplus allocation visits forward with an active gate.
    pub fn redistribute(
        &mut self,
        preferences: &[Preference],
        commodities: &[Commodity; 32],
    ) -> Result<(), String> {
        if preferences.len() > 32
            || !self.balance().is_finite()
            || !self.float(0x8c).is_finite()
            || preferences.iter().any(|p| {
                let Some(c) = commodities.get(p.commodity as usize) else {
                    return true;
                };
                !p.factor.is_finite()
                    || !c.value.is_finite()
                    || !c.markup.is_finite()
                    || !self.float(4 + p.commodity as usize * 4).is_finite()
            })
        {
            return Err("invalid resident redistribution input".into());
        }
        let mut remaining = f64::from(self.balance()) - f64::from(self.float(0x8c));
        let mut mask = u32::from_le_bytes(self.bytes[0x84..0x88].try_into().unwrap());
        let adjust = |account: &mut Self, id: usize, delta: f64, mask: &mut u32| {
            let at = 4 + id * 4;
            let cell = (delta + f64::from(account.float(at))) as f32;
            account.set_float(at, cell);
            account.set_float(0x8c, (delta + f64::from(account.float(0x8c))) as f32);
            if cell == 0. {
                *mask &= !(1 << id);
            } else {
                *mask |= 1 << id;
            }
        };
        if remaining < 0. {
            if self.float(0x8c) == 0. {
                return Ok(());
            }
            for preference in preferences.iter().rev() {
                if remaining >= 0. {
                    break;
                }
                let id = preference.commodity as usize;
                let negative_cell = -f64::from(self.float(4 + id * 4));
                let delta = if negative_cell <= remaining {
                    remaining
                } else {
                    negative_cell
                };
                adjust(self, id, delta, &mut mask);
                remaining -= delta;
            }
        } else {
            mask = 0;
            let mut count = preferences.len() as f64;
            for preference in preferences {
                let id = preference.commodity as usize;
                let commodity = commodities[id];
                let quota = remaining / count;
                let gap = ((f64::from(commodity.markup) * f64::from(commodity.value))
                    * f64::from(preference.factor))
                    - f64::from(self.float(4 + id * 4));
                // FST at0045b24e retains the unrounded gap for comparison.
                // Only the selected gap branch reloads that f32 store.
                let delta = if gap < quota {
                    f64::from(gap as f32)
                } else {
                    quota
                };
                if commodity.active {
                    adjust(self, id, delta, &mut mask);
                }
                count -= 1.;
                remaining -= delta;
            }
        }
        self.bytes[0x84..0x88].copy_from_slice(&mask.to_le_bytes());
        Ok(())
    }

    /// 0045b000. One RNG step per preference, including inactive commodities.
    /// Caller supplies the shared native RNG state; this method does not reseed.
    pub fn allocate(
        &mut self,
        preferences: &[Preference],
        commodities: &[Commodity; 32],
        random_state: &mut u32,
    ) -> Result<(), String> {
        if preferences.len() > 32
            || preferences.iter().any(|p| {
                let Some(c) = commodities.get(p.commodity as usize) else {
                    return true;
                };
                !p.factor.is_finite() || !c.value.is_finite() || !c.markup.is_finite()
            })
        {
            return Err("resident preference exceeds native table or has non-finite inputs".into());
        }
        let mut mask = 0u32;
        let mut total = crate::x87::Sum::default();
        for preference in preferences {
            let id = preference.commodity as usize;
            let commodity = commodities[id];
            *random_state = random_state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            let remainder = ((*random_state >> 16) & 0x7fff) % 101;
            // Normal simulation uses x87 PC64 (64-bit significand). Evaluate
            // the commodity product and random fraction separately, retaining
            // extended precision until each explicit binary32 store.
            let mut amount = crate::x87::Sum::from_f32(commodity.markup)?;
            amount.mul_f32(commodity.value)?;
            amount.mul_f32(preference.factor)?;
            let mut fraction = crate::x87::Sum::from_i32(remainder as i32);
            fraction.mul_f32(f32::from_bits(0x3c23d70a))?;
            amount.mul(fraction);
            if commodity.active {
                let at = 4 + id * 4;
                let mut cell = amount;
                cell.add_f32(self.float(at))?;
                let cell = cell.to_f32();
                self.set_float(at, cell);
                let mut allocated = amount;
                allocated.add_f32(self.float(0x8c))?;
                self.set_float(0x8c, allocated.to_f32());
                if cell == 0. {
                    mask &= !(1 << id);
                } else {
                    mask |= 1 << id;
                }
            }
            // Inactive entries still contribute. Both final aggregates overwrite
            // their previous values; this is not a sum of the updated cells.
            total.add(amount);
        }
        self.bytes[0x84..0x88].copy_from_slice(&mask.to_le_bytes());
        self.set_float(0x88, total.to_f32());
        self.set_float(0x8c, total.to_f32());
        Ok(())
    }
}

/// Minimal live category access used by original resident creation45c3d0.
/// This permits fresh map and population creation to share authoritative rows.
pub trait PlacementCategories {
    fn placement_center(&self, category: usize) -> Option<[f32; 2]>;
    fn increment_resident_count(&mut self, category: usize);
}
impl PlacementCategories for [crate::housing::Category; 52] {
    fn placement_center(&self, category: usize) -> Option<[f32; 2]> {
        self[category].center
    }
    fn increment_resident_count(&mut self, category: usize) {
        self[category].resident_counter = self[category].resident_counter.wrapping_add(1);
    }
}
impl PlacementCategories for [[u8; 0x17c]; 52] {
    fn placement_center(&self, category: usize) -> Option<[f32; 2]> {
        Some(std::array::from_fn(|axis| {
            f32::from_le_bytes(
                self[category][0xe0 + axis * 4..0xe4 + axis * 4]
                    .try_into()
                    .unwrap(),
            )
        }))
    }
    fn increment_resident_count(&mut self, category: usize) {
        let value = u32::from_le_bytes(self[category][0x88..0x8c].try_into().unwrap());
        self[category][0x88..0x8c].copy_from_slice(&value.wrapping_add(1).to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original packed category-counter fixture"]
    fn matches_native_category_counter_invalidation() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_CATEGORY_COUNTER")).unwrap();
        assert_eq!(&data[..8], b"MT45CB70");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let category = word(at) as i32;
            at += 4;
            let before = &data[at..at + 0x150];
            let after = &data[at + 0x150..at + 0x2a0];
            let mut resident = super::Resident::from_bytes(before[..0x14a].try_into().unwrap());
            resident
                .invalidate_category(category.try_into().unwrap())
                .unwrap();
            assert_eq!(
                resident.as_bytes(),
                &after[..0x14a],
                "case {case} category {category}"
            );
            assert_eq!(&before[0x14a..], &after[0x14a..]);
            at += 0x2a0;
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original saved-resident load movement observations"]
    fn matches_native_saved_movement_fixup() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let board =
            crate::board::Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let centers = board.category_geometry().centers;
        let bytes = read("OPENMT2001_NATIVE_SAVED_MOVEMENT");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTLOADM1");
        assert_eq!(word(8), 2);
        let mut at = 12;
        for name in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let data = read(name);
            let save = crate::save::SaveFile::parse(&data).unwrap();
            let residents = save.residents().unwrap();
            at += 32;
            assert_eq!(word(at) as usize, residents.nodes.len());
            at += 4;
            for (slot, node) in residents.nodes.iter().enumerate() {
                assert_eq!(&bytes[at..at + 330], node.as_slice());
                let mut resident = super::Resident::from_bytes(**node);
                let mut auxiliary = bytes[at + 330..at + 614].try_into().unwrap();
                resident
                    .restore_saved_movement(0x10020000, &mut auxiliary, &centers)
                    .unwrap();
                assert_eq!(
                    resident.as_bytes().as_slice(),
                    &bytes[at + 614..at + 944],
                    "resident{slot}"
                );
                assert_eq!(
                    auxiliary.as_slice(),
                    &bytes[at + 944..at + 1228],
                    "auxiliary{slot}"
                );
                at += 1228;
            }
        }
        assert_eq!(at, bytes.len());
    }

    fn empty_shopping_selection<'a>(
        chains: &'a [Vec<Option<usize>>; 52],
        nodes: &'a [Vec<super::ShoppingNode>; 52],
    ) -> super::ShoppingSelection<'a> {
        super::ShoppingSelection {
            chains,
            nodes,
            category_masks: &[0; 52],
            category_factors: &[0.; 52],
            building_categories: &[],
            building_attributes: &[],
            commodities: &[super::Commodity {
                active: false,
                value: 0.,
                markup: 0.,
            }; 32],
            weights: &[],
            distance_disabled: false,
            category_owners: &[None; 52],
            entrances: &[],
            business_distance_weight: 0.,
        }
    }

    #[test]
    #[ignore = "requires original continuous shopping journey"]
    fn matches_native_shopping_trip() {
        check_native_shopping_trip(false, false);
    }

    #[test]
    #[ignore = "requires original scheduler-driven shopping journeys"]
    fn matches_native_scheduled_shopping_trip() {
        check_native_shopping_trip(true, false);
    }

    #[test]
    #[ignore = "requires original scheduler-driven competing resident journeys"]
    fn matches_native_population_shopping() {
        check_native_shopping_trip(true, true);
    }

    fn check_native_shopping_trip(scheduled: bool, population: bool) {
        let variable = if population {
            "OPENMT2001_NATIVE_POPULATION_SHOPPING"
        } else if scheduled {
            "OPENMT2001_NATIVE_SCHEDULED_SHOPPING"
        } else {
            "OPENMT2001_NATIVE_SHOPPING_TRIP"
        };
        let data = std::fs::read(crate::fixtures::path(variable)).unwrap();
        assert_eq!(
            &data[..8],
            if population {
                b"MTSHOP04"
            } else if scheduled {
                b"MTSHOP03"
            } else {
                b"MTSHOP02"
            }
        );
        let reference = if scheduled { 0x6fd7e8 } else { 0x10010000 };
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 3);
        let mut at = 12;
        for case in 0..word(8) {
            let flags = word(at);
            assert_eq!(flags, case);
            let ticks = word(at + 4) as usize;
            let stride = word(at + 8) as usize;
            at += 12;
            assert_eq!(
                stride,
                if population {
                    5942
                } else if scheduled {
                    4334
                } else {
                    4290
                }
            );
            assert!(ticks > 0);
            let data = &data[at..at + (ticks + 1) * stride];
            at += data.len();
            let initial = &data[..stride];
            let mut residents = crate::economy::ResidentAccounts {
                active_count: if population { 3 } else { 1 },
                nodes: vec![super::Resident::from_bytes(
                    initial[..330].try_into().unwrap(),
                )],
                accounts: vec![super::SpendingAccount::from_bytes(
                    initial[1158..1302].try_into().unwrap(),
                )],
            };
            let mut auxiliaries: Vec<[u8; 284]> = vec![initial[330..614].try_into().unwrap()];
            let businesses: [[u8; 544]; 1] = [initial[614..1158].try_into().unwrap()];
            let counts = std::array::from_fn(|i| {
                u32::from_le_bytes(initial[1302 + i * 4..1306 + i * 4].try_into().unwrap())
            });
            let mut lanes: [[u8; 32]; 1] = [initial[1510..1542].try_into().unwrap()];
            let cache = std::array::from_fn(|i| {
                initial[1542 + i * 52..1542 + (i + 1) * 52]
                    .try_into()
                    .unwrap()
            });
            let mut pool_records = vec![initial[4246..4290].try_into().unwrap()];
            if population {
                assert_eq!(
                    u32::from_le_bytes(initial[5938..5942].try_into().unwrap()),
                    3
                );
                for index in 0..2 {
                    let at = 4334 + index * 802;
                    residents.nodes.push(super::Resident::from_bytes(
                        initial[at..at + 330].try_into().unwrap(),
                    ));
                    auxiliaries.push(initial[at + 330..at + 614].try_into().unwrap());
                    residents.accounts.push(super::SpendingAccount::from_bytes(
                        initial[at + 614..at + 758].try_into().unwrap(),
                    ));
                    pool_records.push(initial[at + 758..at + 802].try_into().unwrap());
                }
            }
            let mut pool = super::ResidentAnimationPool::from_records(pool_records).unwrap();
            let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
            let settings = super::ResidentAnimationSettings::from_image(
                &crate::pe::PeImage::parse(&exe).unwrap(),
            )
            .unwrap();
            let mut centers =
                std::array::from_fn(|i| [10000. + i as f32 * 500., 20000. + i as f32 * 500.]);
            centers[0] = [0., 0.];
            centers[1] = if flags == 2 {
                [5000., 5000.]
            } else {
                [100., 0.]
            };
            let geometry =
                crate::routing::CategoryRoutes::from_geometry(&centers, &[[64.; 2]; 52], 256);
            let mut routes =
                crate::routing::CategoryRoutes::from_cache(*geometry.adjacency(), cache, 256);
            let mut chains = std::array::from_fn(|_| Vec::new());
            chains[1] = vec![Some(0)];
            let mut nodes = std::array::from_fn(|_| Vec::new());
            nodes[1] = vec![super::ShoppingNode {
                building: 0,
                position: centers[1],
            }];
            let mut preferences = super::Preferences {
                lists: std::array::from_fn(|_| Vec::new()),
            };
            preferences.lists[3] = vec![super::Preference {
                commodity: 0,
                factor: 3.,
            }];
            let mut factors = [0.; 52];
            factors[1] = 1.;
            let mut topology = crate::shopping::Topology {
                geometry: crate::board::CategoryGeometry {
                    centers,
                    sizes: [[64.; 2]; 52],
                },
                crossing: crate::board::CrossingTopology {
                    category_links: [[0; 4]; 52],
                    road_neighbors: vec![[10; 4]],
                    lane_base: 10,
                },
                building_businesses: vec![Some(0)],
                building_categories: vec![1],
                building_attributes: vec![0],
                entrances: vec![[20., 0.5, -32.]],
                chains,
                nodes,
                category_masks: [1; 52],
                category_factors: factors,
                categories: [crate::category::State::default(); 52],
                category_counts: counts,
            };
            let shopping_settings = crate::shopping::Settings {
                catalogs: crate::shopping::Catalogs {
                    commodities: std::array::from_fn(|i| if i == 0 { vec![0] } else { vec![] }),
                    fractions: std::array::from_fn(|_| vec![]),
                },
                preferences,
                commodities: [super::Commodity {
                    active: true,
                    value: 0.75,
                    markup: 1.,
                }; 32],
                weights: [[1., 0., 0., 1.]; 3],
                business_distance_weights: [1.; 3],
                interaction_delays: [2.4; 45],
                unit_costs: [0.75; 32],
            };
            let mut clock = crate::clock::Clock {
                calendar: [9, 0, 0, 9, 0],
                timing: [12345, 0, 0, 0, 0, 1],
            };
            let mut controls = crate::scheduler::Controls {
                iterations: 1,
                abort: false,
                alternate_script_active: false,
                network_enabled: true,
                network_override: false,
                deadline: u32::MAX,
                gate_32: false,
                gate_32_offset: false,
                before_six_sound: 0,
            };
            let bindings: Vec<_> = (0..residents.nodes.len())
                .map(|index| crate::shopping::ResidentBinding {
                    resident_reference: reference + index as u32 * 0x150,
                    auxiliary_reference: 0x10020000 + index as u32 * 284,
                    auxiliary_slot: index,
                    model_frame_count: 321,
                })
                .collect();
            for (node, binding) in residents.nodes.iter_mut().zip(&bindings) {
                *node = super::Resident::from_runtime_bytes(
                    *node.as_bytes(),
                    0,
                    binding.auxiliary_reference,
                );
            }
            let mut auxiliary_pool =
                super::AuxiliaryPool::from_slots(auxiliaries, bindings.len()).unwrap();
            let mut ledgers = crate::economy::Ledgers {
                residents,
                businesses: businesses.to_vec(),
                players: vec![],
            };
            let mut stages = std::collections::BTreeSet::new();
            for tick in 0..ticks {
                if flags == 1 && tick == 25 {
                    lanes[0][8..12].fill(0);
                }
                stages.insert(u32::from_le_bytes(
                    ledgers.residents.nodes[0].bytes[0xd0..0xd4]
                        .try_into()
                        .unwrap(),
                ));
                let mut dispatch = |event,
                                    clock: &mut crate::clock::Clock,
                                    _: &mut crate::scheduler::Controls|
                 -> Result<u32, String> {
                    let mut population = crate::shopping::Population {
                        postings: &mut Vec::new(),
                        topology: &mut topology,
                        settings: &shopping_settings,
                        routes: &mut routes,
                        lanes: &mut lanes,
                        auxiliary: &mut auxiliary_pool,
                        animations: &mut pool,
                        animation_settings: &settings,
                        bindings: &bindings,
                    };
                    match event {
                        crate::scheduler::Event::Residents => population.tick(
                            &mut ledgers,
                            crate::shopping::PopulationTick {
                                tick: clock.timing[0],
                                boundary: 31,
                                distance_disabled: flags == 2,
                            },
                            |_, event, _, _| panic!("unexpected tick event{event:?}"),
                            |_, helper, _, _| panic!("unexpected shopping helper{helper:?}"),
                        )?,
                        crate::scheduler::Event::Animations => population
                            .advance_animations(&mut ledgers.residents, |slot, _, _| {
                                panic!("unexpected animation controller{slot}")
                            })?,
                        crate::scheduler::Event::AdvanceHour => {
                            panic!("unexpected hourly boundary")
                        }
                        _ => {}
                    }
                    Ok(0)
                };
                if scheduled {
                    crate::scheduler::advance(&mut clock, &mut controls, dispatch).unwrap();
                } else {
                    dispatch(
                        crate::scheduler::Event::Residents,
                        &mut clock,
                        &mut controls,
                    )
                    .unwrap();
                    dispatch(
                        crate::scheduler::Event::Animations,
                        &mut clock,
                        &mut controls,
                    )
                    .unwrap();
                }
                let mut actual = Vec::with_capacity(stride);
                for bytes in [
                    ledgers.residents.nodes[0].bytes.as_slice(),
                    auxiliary_pool.slots()[0].as_slice(),
                    ledgers.businesses[0].as_slice(),
                    ledgers.residents.accounts[0].bytes.as_slice(),
                ] {
                    actual.extend_from_slice(bytes);
                }
                for count in topology.category_counts {
                    actual.extend_from_slice(&count.to_le_bytes());
                }
                actual.extend_from_slice(&lanes[0]);
                for row in routes.cache() {
                    actual.extend_from_slice(row);
                }
                actual.extend_from_slice(&pool.records()[0]);
                if scheduled {
                    for word in clock.calendar.into_iter().chain(clock.timing) {
                        actual.extend_from_slice(&word.to_le_bytes());
                    }
                }
                if population {
                    for (index, auxiliary) in auxiliary_pool.slots().iter().enumerate().skip(1) {
                        actual.extend_from_slice(&ledgers.residents.nodes[index].bytes);
                        actual.extend_from_slice(auxiliary);
                        actual.extend_from_slice(&ledgers.residents.accounts[index].bytes);
                        actual.extend_from_slice(&pool.records()[index]);
                    }
                    actual.extend_from_slice(&ledgers.residents.active_count.to_le_bytes());
                }
                let expected = &data[(tick + 1) * stride..(tick + 2) * stride];
                assert_eq!(actual.len(), stride, "complete tick{tick} snapshot");
                if let Some(offset) = actual.iter().zip(expected).position(|(a, b)| a != b) {
                    panic!(
                        "tick{tick} snapshot offset{offset:x}: actual{:02x} expected{:02x}",
                        actual[offset], expected[offset]
                    );
                }
            }
            for stage in [0, 3, 6, 5] {
                assert!(stages.contains(&stage), "missing shopping stage{stage}");
            }
            assert_eq!(ledgers.residents.nodes[0].bytes[0x7c], 1);
            assert!(u16::from_le_bytes(ledgers.businesses[0][0x80..0x82].try_into().unwrap()) < 8);
            assert!(
                f32::from_le_bytes(
                    ledgers.residents.accounts[0].bytes[4..8]
                        .try_into()
                        .unwrap()
                ) < 10.
            );
            assert_eq!(
                u32::from_le_bytes(
                    ledgers.residents.nodes[0].bytes[0xcc..0xd0]
                        .try_into()
                        .unwrap()
                ),
                12
            );
            if flags == 2 {
                assert!(stages.contains(&4));
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original resident animation observations"]
    fn matches_native_resident_animation() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_ANIMATION")).unwrap();
        assert_eq!(&data[..8], b"MTRANIM2");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        let mut settings = super::ResidentAnimationSettings {
            ranges: [[[0; 2]; 14]; 5],
        };
        for row in &mut settings.ranges {
            for pair in row {
                for value in pair {
                    *value = u16::from_le_bytes(data[at..at + 2].try_into().unwrap());
                    at += 2;
                }
            }
        }
        for case in 0..word(8) {
            let action = word(at);
            at += 4;
            let mut r = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let mut records = Vec::new();
            for _ in 0..4 {
                records.push(data[at..at + 44].try_into().unwrap());
                at += 44;
            }
            let mut pool = super::ResidentAnimationPool::from_records(records).unwrap();
            if action == 256 {
                for index in 0..pool.records().len() {
                    r.advance_animation_slot(
                        &mut auxiliary,
                        &mut pool,
                        index,
                        0x10010000,
                        0x10020000,
                    )
                    .unwrap();
                }
            } else {
                r.request_animation(
                    &mut auxiliary,
                    &mut pool,
                    &settings,
                    super::ResidentAnimationRequest {
                        action,
                        resident_reference: 0x10010000,
                        auxiliary_reference: 0x10020000,
                        model_frame_count: 321,
                    },
                )
                .unwrap();
            }
            for actual in [r.bytes.as_slice(), auxiliary.as_slice()] {
                assert_eq!(actual, &data[at..at + actual.len()], "case{case}");
                at += actual.len();
            }
            for actual in pool.records() {
                assert_eq!(actual.as_slice(), &data[at..at + 44], "case{case}");
                at += 44;
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original traffic crossing observations"]
    fn matches_native_crossing_traffic() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_CROSSING_TRAFFIC")).unwrap();
        assert_eq!(&data[..8], b"MTCROSS1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let entry = word(at);
            let missing = word(at + 4) != 0;
            let result = word(at + 8) != 0;
            at += 12;
            let mut r = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut lanes = [[0; 32]; 11];
            lanes[10].copy_from_slice(&data[at..at + 32]);
            at += 32;
            let links = [[if missing { 255 } else { 3 }; 4]; 52];
            let mut junctions = [[0; 4]; 4];
            junctions[3].fill(20);
            let mut world = super::CrossingWorld {
                category_links: &links,
                road_neighbors: &junctions,
                lane_base: 10,
                lanes: &mut lanes,
            };
            match entry {
                0x45d5f0 => assert_eq!(r.crossing_allowed(&world).unwrap(), result, "case{case}"),
                0x45d6c0 => r.set_crossing_active(&mut world, true).unwrap(),
                0x45d7a0 => r.set_crossing_active(&mut world, false).unwrap(),
                _ => panic!("unknown entry"),
            }
            assert_eq!(r.bytes.as_slice(), &data[at..at + 330], "case{case}");
            at += 330;
            assert_eq!(lanes[10].as_slice(), &data[at..at + 32], "case{case}");
            at += 32;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original complete category trips"]
    fn matches_native_category_trip() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_CATEGORY_TRIP")).unwrap();
        assert_eq!(&data[..8], b"MTCATRP5");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        let mut settings = super::ResidentAnimationSettings {
            ranges: [[[0; 2]; 14]; 5],
        };
        for row in &mut settings.ranges {
            for pair in row {
                for value in pair {
                    *value = u16::from_le_bytes(data[at..at + 2].try_into().unwrap());
                    at += 2;
                }
            }
        }
        for case in 0..word(8) {
            let direction = word(at) as usize;
            let fallback = word(at + 4) != 0;
            let blocked = word(at + 8) != 0;
            let ticks = word(at + 12);
            at += 16;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let mut counts = [0; 52];
            for count in &mut counts[..3] {
                *count = word(at);
                at += 4;
            }
            let mut lanes: [[u8; 32]; 1] = [data[at..at + 32].try_into().unwrap()];
            at += 32;
            let mut pool = super::ResidentAnimationPool::from_records(vec![
                data[at..at + 44].try_into().unwrap(),
            ])
            .unwrap();
            at += 44;
            let paths = [
                [[0., 0.], [100., 0.], [100., 100.]],
                [[0., 0.], [-100., 0.], [-100., -100.]],
                [[0., 0.], [0., 100.], [-100., 100.]],
                [[0., 0.], [0., -100.], [100., -100.]],
            ];
            let mut centers = [[0.; 2]; 52];
            centers[..3].copy_from_slice(&paths[direction]);
            if fallback {
                centers[1] = [5000., 5000.];
            }
            for (i, center) in centers.iter_mut().enumerate().skip(3) {
                *center = [10000. + i as f32 * 500., 20000. + i as f32 * 500.];
            }
            let mut routes =
                crate::routing::CategoryRoutes::from_geometry(&centers, &[[64.; 2]; 52], 256);
            for row in routes.cache() {
                assert_eq!(row.as_slice(), &data[at..at + 52]);
                at += 52;
            }
            let mut junctions = [[0; 4]];
            junctions[0].fill(10);
            let chains = std::array::from_fn(|_| Vec::new());
            let nodes = std::array::from_fn(|_| Vec::new());
            let preferences = super::Preferences {
                lists: std::array::from_fn(|_| Vec::new()),
            };
            let retail = super::RetailWorld {
                building_businesses: &[],
                catalogs: &[],
                interaction_delays: &[],
                unit_costs: &[0.; 32],
                preferences: &preferences,
                tick: 0,
            };
            for tick in 0..ticks {
                if blocked && tick == 25 {
                    lanes[0][8..12].fill(0);
                }
                let n = word(at) as usize;
                at += 4;
                let expected_calls: Vec<_> = (0..n).map(|i| word(at + i * 4)).collect();
                at += n * 4;
                let mut calls = Vec::new();
                resident
                    .tick_with_shopping(
                        &mut auxiliary,
                        &mut super::ShoppingState {
                            local: super::LocalPurchaseState {
                                postings: &mut Vec::new(),
                                retail: &retail,
                                businesses: &mut [],
                                accounts: &mut [],
                                owners: &mut [],
                                centers: &centers,
                                boundary: 31,
                                category_counts: &mut counts,
                            },
                            selection: empty_shopping_selection(&chains, &nodes),
                            routes: &mut routes,
                            crossing: &mut super::CrossingWorld {
                                category_links: &[[0; 4]; 52],
                                road_neighbors: &junctions,
                                lane_base: 10,
                                lanes: &mut lanes,
                            },
                        },
                        |mode, r, a| {
                            calls.push(mode);
                            r.request_animation(
                                a,
                                &mut pool,
                                &settings,
                                super::ResidentAnimationRequest {
                                    action: mode,
                                    resident_reference: 0x10010000,
                                    auxiliary_reference: 0x10020000,
                                    model_frame_count: 321,
                                },
                            )
                        },
                        |event, _, _| panic!("unexpected {event:?}"),
                        |helper, _, _| panic!("unexpected {helper:?}"),
                    )
                    .unwrap();
                resident
                    .advance_animation_slot(&mut auxiliary, &mut pool, 0, 0x10010000, 0x10020000)
                    .unwrap();
                assert_eq!(calls, expected_calls, "case{case} tick{tick}");
                for actual in [resident.bytes.as_slice(), auxiliary.as_slice()] {
                    assert_eq!(
                        actual,
                        &data[at..at + actual.len()],
                        "case{case} tick{tick} offset{at}"
                    );
                    at += actual.len();
                }
                for count in &counts[..3] {
                    assert_eq!(*count, word(at), "case{case} tick{tick}");
                    at += 4;
                }
                assert_eq!(
                    lanes[0].as_slice(),
                    &data[at..at + 32],
                    "case{case} tick{tick}"
                );
                at += 32;
                assert_eq!(
                    pool.records()[0].as_slice(),
                    &data[at..at + 44],
                    "case{case} tick{tick}"
                );
                at += 44;
                for row in routes.cache() {
                    assert_eq!(
                        row.as_slice(),
                        &data[at..at + 52],
                        "case{case} tick{tick} cache"
                    );
                    at += 52;
                }
            }
            assert_eq!(resident.bytes[0x7c], 2);
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original category traversal observations"]
    fn matches_native_category_travel() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_CATEGORY_TRAVEL")).unwrap();
        assert_eq!(&data[..8], b"MTCATTR1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let next = word(at) as u8;
            let gate = word(at + 4) != 0;
            let result = word(at + 8) != 0;
            let call_count = word(at + 12) as usize;
            at += 16;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut counts = [0; 52];
            counts[0] = word(at);
            counts[1] = word(at + 4);
            at += 8;
            let expected = &data[at..at + 330];
            at += 330;
            let expected_counts = [word(at), word(at + 4)];
            at += 8;
            let mut centers = [[0.; 2]; 52];
            centers[1] = [100., 0.];
            let seen = std::cell::Cell::new(0_usize);
            let check = |address: u32, r: Option<&super::Resident>| {
                let i = seen.get();
                assert!(i < call_count);
                assert_eq!(word(at + i * 334), address, "case{case}");
                if let Some(r) = r {
                    assert_eq!(
                        r.bytes.as_slice(),
                        &data[at + i * 334 + 4..at + (i + 1) * 334],
                        "callback case{case}"
                    );
                }
                seen.set(i + 1);
            };
            assert_eq!(
                resident
                    .advance_category_travel(
                        31,
                        &centers,
                        &mut counts,
                        |_, _| {
                            check(0x45ebd0, None);
                            Ok(next)
                        },
                        |event, r| {
                            let address = match event {
                                super::CrossingEvent::Prepare => 0x45d140,
                                super::CrossingEvent::Gate => 0x45d5f0,
                                super::CrossingEvent::Animation(2) => 0x45ce10,
                                super::CrossingEvent::Begin => 0x45d6c0,
                                super::CrossingEvent::Finish => 0x45d7a0,
                                super::CrossingEvent::WaitingAnimation => 0x45d520,
                                _ => panic!("unexpected {event:?}"),
                            };
                            check(address, Some(r));
                            Ok(event == super::CrossingEvent::Gate && gate)
                        }
                    )
                    .unwrap(),
                result,
                "case{case}"
            );
            assert_eq!(seen.get(), call_count);
            assert_eq!(resident.bytes.as_slice(), expected, "case{case}");
            assert_eq!(counts[..2], expected_counts, "case{case}");
            at += call_count * 334;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original complete local purchase trips"]
    fn matches_native_local_purchase_trip() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_LOCAL_PURCHASE_TRIP")).unwrap();
        assert_eq!(&data[..8], b"MTTRIP01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let ticks = word(at);
            at += 4;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let mut businesses: [[u8; 544]; 1] = [data[at..at + 544].try_into().unwrap()];
            at += 544;
            let mut accounts = [super::SpendingAccount::from_bytes(
                data[at..at + 144].try_into().unwrap(),
            )];
            at += 144;
            let mut counts = [0; 52];
            counts[0] = word(at);
            at += 4;
            let preferences = super::Preferences {
                lists: std::array::from_fn(|_| {
                    vec![
                        super::Preference {
                            commodity: 31,
                            factor: 0.,
                        },
                        super::Preference {
                            commodity: 7,
                            factor: 3.,
                        },
                    ]
                }),
            };
            let retail = super::RetailWorld {
                building_businesses: &[Some(0)],
                catalogs: &[vec![31, 7]],
                interaction_delays: &[2.4],
                unit_costs: &[0.75; 32],
                preferences: &preferences,
                tick: 12345,
            };
            let chains = std::array::from_fn(|_| Vec::new());
            let nodes = std::array::from_fn(|_| Vec::new());
            let mut routes = crate::routing::CategoryRoutes::new([[0; 52]; 52], 256);
            for tick in 0..ticks {
                resident
                    .tick_with_shopping(
                        &mut auxiliary,
                        &mut super::ShoppingState {
                            local: super::LocalPurchaseState {
                                postings: &mut Vec::new(),
                                retail: &retail,
                                centers: &[[0.; 2]; 52],
                                boundary: 31,
                                category_counts: &mut counts,
                                businesses: &mut businesses,
                                accounts: &mut accounts,
                                owners: &mut [],
                            },
                            selection: empty_shopping_selection(&chains, &nodes),
                            routes: &mut routes,
                            crossing: &mut super::CrossingWorld {
                                category_links: &[[255; 4]; 52],
                                road_neighbors: &[],
                                lane_base: 0,
                                lanes: &mut [],
                            },
                        },
                        |mode, _, _| panic!("unexpected animation{mode}"),
                        |event, _, _| panic!("unexpected {event:?}"),
                        |helper, _, _| panic!("unexpected {helper:?}"),
                    )
                    .unwrap();
                for actual in [
                    resident.bytes.as_slice(),
                    auxiliary.as_slice(),
                    businesses[0].as_slice(),
                    accounts[0].bytes.as_slice(),
                ] {
                    assert_eq!(
                        actual,
                        &data[at..at + actual.len()],
                        "case{case} tick{tick} offset{at}"
                    );
                    at += actual.len();
                }
                assert_eq!(counts[0], word(at), "case{case} tick{tick}");
                at += 4;
            }
            assert!(u16::from_le_bytes(businesses[0][0xb4..0xb6].try_into().unwrap()) < 8);
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original entrance approach observations"]
    fn matches_native_entrance_approach() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_ENTRANCE_APPROACH")).unwrap();
        assert_eq!(&data[..8], b"MTAPPR01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let done = word(at) != 0;
            let result = word(at + 4) != 0;
            let count = word(at + 8) as usize;
            at += 12;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let mut calls = 0;
            assert_eq!(
                resident
                    .advance_entrance_approach(31, &[[1.25, -2.5]; 52], |r| {
                        assert_eq!(
                            r.bytes.as_slice(),
                            &data[at + calls * 330..at + (calls + 1) * 330],
                            "callback case{case}"
                        );
                        calls += 1;
                        for (offset, value) in [(0xd8, 12_u32), (0xdc, 1), (0xe0, 23), (0xe4, 29)] {
                            r.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                        }
                        r.bytes[0x7e] = 0xb5;
                        Ok(done)
                    })
                    .unwrap(),
                result,
                "case{case}"
            );
            assert_eq!(calls, count, "case{case}");
            assert_eq!(resident.bytes.as_slice(), expected, "case{case}");
            at += count * 330;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original repeated movement observations"]
    fn matches_native_resident_movement() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_MOVEMENT")).unwrap();
        assert_eq!(&data[..8], b"MTMOTION");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let tick = word(at) != 0;
            let result = word(at + 4) != 0;
            let expected_calls = word(at + 8);
            at += 12;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let mut calls = 0;
            if tick {
                resident
                    .tick(
                        &mut auxiliary,
                        &[[1.25, -2.5]; 52],
                        |event, resident, auxiliary| match event {
                            super::TickEvent::Movement => Ok(resident.advance_movement(auxiliary)),
                            super::TickEvent::Action => {
                                calls += 1;
                                Ok(false)
                            }
                            _ => panic!("unexpected event {event:?}"),
                        },
                    )
                    .unwrap();
            } else {
                assert_eq!(
                    resident.advance_movement(&mut auxiliary),
                    result,
                    "case{case}"
                );
            }
            assert_eq!(calls, expected_calls, "case{case}");
            assert_eq!(
                resident.bytes.as_slice(),
                &data[at..at + 330],
                "resident case{case}"
            );
            at += 330;
            assert_eq!(
                auxiliary.as_slice(),
                &data[at..at + 284],
                "auxiliary case{case}"
            );
            at += 284;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original local target observations"]
    fn matches_native_local_target() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_LOCAL_TARGET")).unwrap();
        assert_eq!(&data[..8], b"MTLOCAL2");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let native_motion = word(at) != 0;
            at += 4;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let counter = word(at);
            let result = word(at + 4) != 0;
            let count = word(at + 8) as usize;
            at += 12;
            let calls: Vec<_> = (0..count)
                .map(|i| (word(at + i * 8), word(at + i * 8 + 4) != 0))
                .collect();
            at += count * 8;
            let mut counts = [0x13579bdf; 52];
            let mut seen = 0;
            assert_eq!(
                resident
                    .advance_local_target(&mut counts, |helper, resident| {
                        assert_eq!(helper as u32, calls[seen].0);
                        let result = calls[seen].1;
                        seen += 1;
                        if native_motion {
                            resident.schedule_local_movement(&[[1.25, -2.5]; 52])
                        } else {
                            Ok(result)
                        }
                    })
                    .unwrap(),
                result,
                "case{case}"
            );
            assert_eq!(seen, count);
            assert_eq!(counts[0], counter, "case{case}");
            assert_eq!(resident.bytes.as_slice(), expected, "case{case}");
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original selected transaction observations"]
    fn matches_native_selected_transaction() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_SELECTED_TRANSACTION")).unwrap();
        assert_eq!(&data[..8], b"MTSELBUY");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let tick = word(at) != 0;
            let variant = word(at + 4);
            at += 8;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut businesses = [[0; 544]; 3];
            businesses[2].copy_from_slice(&data[at..at + 544]);
            at += 544;
            let mut accounts = [super::SpendingAccount::from_bytes(
                data[at..at + 144].try_into().unwrap(),
            )];
            at += 144;
            let expected_resident = &data[at..at + 330];
            at += 330;
            let expected_business = &data[at..at + 544];
            at += 544;
            let expected_account = &data[at..at + 144];
            at += 144;
            let mut links = [None; 4];
            links[3] = (!matches!(variant, 1 | 2)).then_some(2);
            let mut catalogs = vec![vec![]; 5];
            catalogs[4] = vec![31, if variant == 4 { 8 } else { 7 }];
            let preferences = super::Preferences {
                lists: std::array::from_fn(|_| {
                    vec![
                        super::Preference {
                            commodity: 31,
                            factor: 0.,
                        },
                        super::Preference {
                            commodity: 7,
                            factor: 3.,
                        },
                    ]
                }),
            };
            let world = super::RetailWorld {
                building_businesses: &links,
                catalogs: &catalogs,
                interaction_delays: &[2.4; 5],
                unit_costs: &[0.75; 32],
                preferences: &preferences,
                tick: 12345,
            };
            if tick {
                let mut auxiliary = [0; 284];
                let mut counts = [9; 52];
                resident
                    .tick_with_purchase_actions(
                        &mut auxiliary,
                        &[[0.; 2]; 52],
                        |event, _, _| panic!("unexpected {event:?}"),
                        |helper, resident, _| match helper {
                            super::ActionHelper::At45e680 => resident
                                .advance_local_target(&mut counts, |helper, _| {
                                    panic!("unexpected {helper:?}")
                                }),
                            super::ActionHelper::At45f660 => {
                                resident.purchase_selected(
                                    &world,
                                    &mut businesses,
                                    &mut accounts,
                                    &mut [],
                                )?;
                                Ok(false)
                            }
                            _ => panic!("unexpected {helper:?}"),
                        },
                    )
                    .unwrap();
                assert_eq!(counts[0], 8);
            } else {
                resident
                    .purchase_selected(&world, &mut businesses, &mut accounts, &mut [])
                    .unwrap();
            }
            assert_eq!(
                resident.bytes.as_slice(),
                expected_resident,
                "case{case} resident"
            );
            assert_eq!(
                businesses[2].as_slice(),
                expected_business,
                "case{case} business"
            );
            assert_eq!(
                accounts[0].bytes.as_slice(),
                expected_account,
                "case{case} account"
            );
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original business selection observations"]
    fn matches_native_purchase_business() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_BUSINESS")).unwrap();
        assert_eq!(&data[..8], b"MTPBUS01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let double = |at| f64::from_le_bytes(data[at..at + 8].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let scores = [double(at), double(at + 8)];
            let weight = f32::from_bits(word(at + 16));
            at += 20;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let success = word(at) != 0;
            let finished = word(at + 4);
            let count = word(at + 8) as usize;
            at += 12;
            let expected_calls: Vec<_> = (0..count)
                .map(|i| (word(at + i * 8) as usize, word(at + i * 8 + 4)))
                .collect();
            at += count * 8;
            let nodes = [
                super::PurchaseNode {
                    building: 7,
                    position: [99., 0.],
                    business_mask: Some(3),
                },
                super::PurchaseNode {
                    building: 8,
                    position: [0.; 2],
                    business_mask: Some(3),
                },
            ];
            let mut calls = Vec::new();
            let mut finishes = 0;
            let entrances: [[f32; 3]; 9] =
                std::array::from_fn(|i| [i as f32 * 1.25, 99., -(i as f32) * 2.5]);
            let result = resident
                .select_purchase_business(
                    super::PurchaseBusinessInputs {
                        preferences: &[1, 0],
                        account_mask: 3,
                        nodes: &nodes,
                        auxiliary_position: [0.; 2],
                        distance_weight: weight,
                    },
                    |_, i, c| {
                        calls.push((i, c));
                        Ok(scores[i])
                    },
                    |resident| {
                        finishes += 1;
                        resident.finish_purchase_destination(&entrances)
                    },
                )
                .unwrap();
            assert_eq!(result, success, "case{case}");
            assert_eq!(resident.bytes.as_slice(), expected, "case{case}");
            assert_eq!(calls, expected_calls, "case{case}");
            assert_eq!(finishes, finished, "case{case}");
        }
        assert_eq!(at, data.len());
    }
    #[test]
    fn business_selection_respects_preference_order_and_conditional_distance() {
        let mut resident = super::Resident::from_bytes([0; 330]);
        resident.bytes[0x6c..0x70].copy_from_slice(&3_u32.to_le_bytes());
        let nodes = [
            super::PurchaseNode {
                building: 7,
                position: [99., 0.],
                business_mask: Some(3),
            },
            super::PurchaseNode {
                building: 8,
                position: [0., 0.],
                business_mask: Some(3),
            },
        ];
        let mut calls = Vec::new();
        assert!(
            resident
                .select_purchase_business(
                    super::PurchaseBusinessInputs {
                        preferences: &[1, 0],
                        account_mask: 3,
                        nodes: &nodes,
                        auxiliary_position: [0.; 2],
                        distance_weight: 1.,
                    },
                    |_, node, commodity| {
                        calls.push((node, commodity));
                        Ok(if node == 0 { 10. } else { 12. })
                    },
                    |resident| {
                        assert_eq!(
                            i16::from_le_bytes(resident.bytes[0x46..0x48].try_into().unwrap()),
                            8
                        );
                        Ok(())
                    }
                )
                .unwrap()
        );
        assert_eq!(calls, vec![(0, 1), (1, 1)]);
        assert_eq!(
            u32::from_le_bytes(resident.bytes[0x64..0x68].try_into().unwrap()),
            1
        );
        // First gets10+1=11; second raw12 differs by exactly1, so no bonus.
        // A zero score must not receive even a large proximity bonus.
        assert!(
            !resident
                .select_purchase_business(
                    super::PurchaseBusinessInputs {
                        preferences: &[0],
                        account_mask: 3,
                        nodes: &nodes,
                        auxiliary_position: [0.; 2],
                        distance_weight: 100.,
                    },
                    |_, _, _| Ok(0.),
                    |_| panic!("unexpected destination")
                )
                .unwrap()
        );
    }
    #[test]
    #[ignore = "requires composed original purchase chooser observations"]
    fn matches_native_purchase_business_composed() {
        let data =
            std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_BUSINESS_COMPOSED")).unwrap();
        assert_eq!(&data[..8], b"MTPBOTH2");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let mode = word(at);
            let disabled = word(at + 4) != 0;
            let busy = word(at + 8);
            at += 16;
            let before: [u8; 330] = data[at..at + 330].try_into().unwrap();
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let result = word(at) != 0;
            at += 4;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let expected_auxiliary = &data[at..at + 284];
            at += 284;
            let mut resident = super::Resident::from_bytes([0; 330]);
            resident.bytes[0x2c] = 0x14; // categories1 and2 eligible
            resident.bytes[0x6c..0x70].copy_from_slice(&1_u32.to_le_bytes());
            resident.bytes[0x70..0x74].copy_from_slice(&10_f32.to_le_bytes());
            resident.bytes[0xe..0x10].copy_from_slice(&(-1_i16).to_le_bytes());
            let mut account = super::SpendingAccount::from_bytes([0; 144]);
            account.bytes[4..8].copy_from_slice(&10_f32.to_le_bytes());
            account.bytes[0x84..0x88].copy_from_slice(&1_u32.to_le_bytes());
            let mut accounts = [account];
            let mut businesses = [[0; 544]; 2];
            for (i, b) in businesses.iter_mut().enumerate() {
                b[8..10].copy_from_slice(&(i as i16).to_le_bytes());
                b[0x70..0x74].copy_from_slice(&1_u32.to_le_bytes());
                b[0x218..0x21c].copy_from_slice(&1_u32.to_le_bytes());
                b[0x1c4..0x1c8]
                    .copy_from_slice(&(if i == 0 { 2_f32 } else { 1_f32 }).to_le_bytes());
            }
            businesses[1][0x6c..0x70].copy_from_slice(&busy.to_le_bytes());
            let catalogs = [vec![0]];
            let mut chains: [Vec<Option<usize>>; 52] = std::array::from_fn(|_| vec![]);
            chains[1] = vec![Some(0), Some(1)];
            chains[2] = vec![Some(1)];
            let mut centers = [[0.; 2]; 52];
            centers[2] = [5000., 0.];
            let masks = [1; 52];
            let factors = [0.; 52];
            let categories = [1, 2];
            let attributes = [0, 0];
            let commodities = [super::Commodity {
                active: true,
                value: 1.,
                markup: 1.,
            }; 32];
            let weights = [[1., 0., 0., 2.]];
            let world = super::PurchaseWorld {
                businesses: &businesses,
                catalogs: &catalogs,
                chains: &chains,
                category_masks: &masks,
                category_factors: &factors,
                centers: &centers,
                building_categories: &categories,
                building_attributes: &attributes,
                commodities: &commodities,
                weights: &weights,
                distance_disabled: disabled,
            };

            resident = super::Resident::from_bytes(before);
            if mode == 0 {
                assert_eq!(
                    resident
                        .choose_purchase_business(
                            &world,
                            &accounts,
                            super::PurchaseBusinessInputs {
                                preferences: &[0],
                                account_mask: 1,
                                nodes: &[
                                    super::PurchaseNode {
                                        building: 0,
                                        position: [0.; 2],
                                        business_mask: Some(1)
                                    },
                                    super::PurchaseNode {
                                        building: 1,
                                        position: [0.; 2],
                                        business_mask: Some(1)
                                    }
                                ],
                                auxiliary_position: [0.; 2],
                                distance_weight: 1.,
                            },
                            &[[12.25, 99., -5.5], [13.25, 99., -6.5]]
                        )
                        .unwrap(),
                    result,
                    "case{case}"
                );
            } else {
                let mut nodes = std::array::from_fn(|_| Vec::new());
                nodes[1] = vec![
                    super::ShoppingNode {
                        building: 0,
                        position: [0.; 2],
                    },
                    super::ShoppingNode {
                        building: 1,
                        position: [0.; 2],
                    },
                ];
                nodes[2] = vec![super::ShoppingNode {
                    building: 1,
                    position: [0.; 2],
                }];
                let preferences = super::Preferences {
                    lists: std::array::from_fn(|_| {
                        vec![super::Preference {
                            commodity: 0,
                            factor: 0.,
                        }]
                    }),
                };
                let retail = super::RetailWorld {
                    building_businesses: &[Some(0), Some(1)],
                    catalogs: &catalogs,
                    interaction_delays: &[],
                    unit_costs: &[0.; 32],
                    preferences: &preferences,
                    tick: 0,
                };
                resident
                    .tick_with_shopping(
                        &mut auxiliary,
                        &mut super::ShoppingState {
                            local: super::LocalPurchaseState {
                                postings: &mut Vec::new(),
                                retail: &retail,
                                centers: &centers,
                                boundary: 31,
                                category_counts: &mut [0; 52],
                                businesses: &mut businesses,
                                accounts: &mut accounts,
                                owners: &mut [],
                            },
                            selection: super::ShoppingSelection {
                                chains: &chains,
                                category_masks: &masks,
                                category_factors: &factors,
                                building_categories: &categories,
                                building_attributes: &attributes,
                                commodities: &commodities,
                                weights: &weights,
                                distance_disabled: disabled,
                                category_owners: &[None; 52],
                                nodes: &nodes,
                                entrances: &[[12.25, 99., -5.5], [13.25, 99., -6.5]],
                                business_distance_weight: 1.,
                            },
                            routes: &mut crate::routing::CategoryRoutes::new([[0; 52]; 52], 256),
                            crossing: &mut super::CrossingWorld {
                                category_links: &[[255; 4]; 52],
                                road_neighbors: &[],
                                lane_base: 0,
                                lanes: &mut [],
                            },
                        },
                        |mode, _, _| panic!("unexpected animation{mode}"),
                        |event, _, _| panic!("unexpected tick {event:?}"),
                        |helper, _, _| panic!("unexpected action {helper:?}"),
                    )
                    .unwrap();
            }
            assert_eq!(
                auxiliary.as_slice(),
                expected_auxiliary,
                "case{case} auxiliary"
            );
            assert_eq!(resident.bytes.as_slice(), expected, "case{case}");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires composed original purchase chooser observations"]
    fn matches_native_purchase_composed() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_COMPOSED")).unwrap();
        assert_eq!(&data[..8], b"MTPCOMP1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let disabled = word(at) != 0;
            let busy = word(at + 4);
            at += 12;
            let before: [u8; 330] = data[at..at + 330].try_into().unwrap();
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let result = word(at) != 0;
            at += 4;
            let mut resident = super::Resident::from_bytes([0; 330]);
            resident.bytes[0x2c] = 0x14; // categories1 and2 eligible
            resident.bytes[0x6c..0x70].copy_from_slice(&1_u32.to_le_bytes());
            resident.bytes[0x70..0x74].copy_from_slice(&10_f32.to_le_bytes());
            resident.bytes[0xe..0x10].copy_from_slice(&(-1_i16).to_le_bytes());
            let mut account = super::SpendingAccount::from_bytes([0; 144]);
            account.bytes[4..8].copy_from_slice(&10_f32.to_le_bytes());
            account.bytes[0x84..0x88].copy_from_slice(&1_u32.to_le_bytes());
            let accounts = [account];
            let mut businesses = [[0; 544]; 2];
            for (i, b) in businesses.iter_mut().enumerate() {
                b[8..10].copy_from_slice(&(i as i16).to_le_bytes());
                b[0x70..0x74].copy_from_slice(&1_u32.to_le_bytes());
                b[0x218..0x21c].copy_from_slice(&1_u32.to_le_bytes());
                b[0x1c4..0x1c8]
                    .copy_from_slice(&(if i == 0 { 2_f32 } else { 1_f32 }).to_le_bytes());
            }
            businesses[1][0x6c..0x70].copy_from_slice(&busy.to_le_bytes());
            let catalogs = [vec![0]];
            let mut chains: [Vec<Option<usize>>; 52] = std::array::from_fn(|_| vec![]);
            chains[1] = vec![None, Some(0)];
            chains[2] = vec![Some(1)];
            let mut centers = [[0.; 2]; 52];
            centers[2] = [5000., 0.];
            let masks = [1; 52];
            let factors = [0.; 52];
            let categories = [1, 2];
            let attributes = [0, 0];
            let commodities = [super::Commodity {
                active: true,
                value: 1.,
                markup: 1.,
            }; 32];
            let weights = [[1., 0., 0., 2.]];
            let world = super::PurchaseWorld {
                businesses: &businesses,
                catalogs: &catalogs,
                chains: &chains,
                category_masks: &masks,
                category_factors: &factors,
                centers: &centers,
                building_categories: &categories,
                building_attributes: &attributes,
                commodities: &commodities,
                weights: &weights,
                distance_disabled: disabled,
            };

            resident = super::Resident::from_bytes(before);
            assert_eq!(
                resident
                    .choose_purchase_category(&world, &accounts, [0.; 2], &[None; 52], &mut [])
                    .unwrap(),
                result,
                "case{case}"
            );
            assert_eq!(resident.bytes.as_slice(), expected, "case{case}");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    fn purchase_choice_combines_prices_and_distance() {
        let mut resident = super::Resident::from_bytes([0; 330]);
        resident.bytes[0x2c] = 0x14; // categories1 and2 eligible
        resident.bytes[0x6c..0x70].copy_from_slice(&1_u32.to_le_bytes());
        resident.bytes[0x70..0x74].copy_from_slice(&10_f32.to_le_bytes());
        resident.bytes[0xe..0x10].copy_from_slice(&(-1_i16).to_le_bytes());
        let mut account = super::SpendingAccount::from_bytes([0; 144]);
        account.bytes[4..8].copy_from_slice(&10_f32.to_le_bytes());
        account.bytes[0x84..0x88].copy_from_slice(&1_u32.to_le_bytes());
        let accounts = [account];
        let mut businesses = [[0; 544]; 2];
        for (i, b) in businesses.iter_mut().enumerate() {
            b[8..10].copy_from_slice(&(i as i16).to_le_bytes());
            b[0x70..0x74].copy_from_slice(&1_u32.to_le_bytes());
            b[0x218..0x21c].copy_from_slice(&1_u32.to_le_bytes());
            b[0x1c4..0x1c8].copy_from_slice(&(if i == 0 { 2_f32 } else { 1_f32 }).to_le_bytes());
        }
        let catalogs = [vec![0]];
        let mut chains: [Vec<Option<usize>>; 52] = std::array::from_fn(|_| vec![]);
        chains[1] = vec![None, Some(0)];
        chains[2] = vec![Some(1)];
        let mut centers = [[0.; 2]; 52];
        centers[2] = [5000., 0.];
        let masks = [1; 52];
        let factors = [0.; 52];
        let categories = [1, 2];
        let attributes = [0, 0];
        let commodities = [super::Commodity {
            active: true,
            value: 1.,
            markup: 1.,
        }; 32];
        let weights = [[1., 0., 0., 2.]];
        let mut world = super::PurchaseWorld {
            businesses: &businesses,
            catalogs: &catalogs,
            chains: &chains,
            category_masks: &masks,
            category_factors: &factors,
            centers: &centers,
            building_categories: &categories,
            building_attributes: &attributes,
            commodities: &commodities,
            weights: &weights,
            distance_disabled: true,
        };
        assert!(
            resident
                .choose_purchase_category(&world, &accounts, [0.; 2], &[None; 52], &mut [])
                .unwrap()
        );
        assert_eq!(resident.bytes[0x44], 2); // cheaper business wins
        world.distance_disabled = false;
        assert!(
            resident
                .choose_purchase_category(&world, &accounts, [0.; 2], &[None; 52], &mut [])
                .unwrap()
        );
        assert_eq!(resident.bytes[0x44], 1); // distance outweighs lower price
        assert_eq!(resident.bytes[0x74], 2);
        // Once movement puts the resident in the selected category, resolve
        // its business using actual prices and complete that building's target.
        resident.bytes[0x7c] = 1;
        let nodes = [
            super::PurchaseNode {
                building: -1,
                position: [0.; 2],
                business_mask: None,
            },
            super::PurchaseNode {
                building: 0,
                position: [10., 0.],
                business_mask: Some(1),
            },
        ];
        assert!(
            resident
                .choose_purchase_business(
                    &world,
                    &accounts,
                    super::PurchaseBusinessInputs {
                        preferences: &[0],
                        account_mask: 1,
                        nodes: &nodes,
                        auxiliary_position: [0.; 2],
                        distance_weight: 1.,
                    },
                    &[[12.25, 9., -5.5], [0.; 3]]
                )
                .unwrap()
        );
        assert_eq!(
            i16::from_le_bytes(resident.bytes[0x46..0x48].try_into().unwrap()),
            0
        );
        let target: [f32; 3] = std::array::from_fn(|i| {
            f32::from_le_bytes(
                resident.bytes[0x48 + i * 4..0x4c + i * 4]
                    .try_into()
                    .unwrap(),
            )
        });
        assert_eq!(target, [12.25, 0.5, -5.5]);
    }
    #[test]
    #[ignore = "requires original distance adjustment observations"]
    fn matches_native_purchase_distance() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_DISTANCE")).unwrap();
        assert_eq!(&data[..8], b"MTDIST01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let disabled = word(at) != 0;
            let category = word(at + 4) as usize;
            let auxiliary = [f32::from_bits(word(at + 8)), f32::from_bits(word(at + 12))];
            let weight = f32::from_bits(word(at + 16));
            at += 20;
            let centers = std::array::from_fn(|i| {
                [
                    f32::from_bits(word(at + i * 8)),
                    f32::from_bits(word(at + i * 8 + 4)),
                ]
            });
            at += 416;
            let expected = f64::from_le_bytes(data[at..at + 8].try_into().unwrap());
            at += 8;
            assert_eq!(
                resident
                    .purchase_distance_adjustment(category, &centers, auxiliary, weight, disabled)
                    .unwrap()
                    .to_bits(),
                expected.to_bits(),
                "case{case}"
            );
        }
        assert_eq!(at, data.len());
    }
    #[test]
    fn purchase_lookup_native_cases() {
        // Original00462960/00462f30 observations rerun2026-09-12.
        let mut business = [0; 544];
        for (at, value) in [(0x1c0, 77.75_f32), (0x1c4, 10.5), (0x1c8, 20.25)] {
            business[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (catalog, commodity, expected) in [
            (&[7, 9][..], 7, 10.5_f32),
            (&[7, 9][..], 9, 20.25),
            (&[7, 9][..], 31, 77.75),
            (&[][..], 7, 77.75),
            (&[7, 7][..], 7, 10.5),
        ] {
            assert_eq!(
                super::purchase_price(&business, catalog, commodity)
                    .unwrap()
                    .to_bits(),
                expected.to_bits()
            );
        }
        for (index, expected) in [(0_i16, 0x1234_i16), (1, -32767), (-1, -1)] {
            business[8..10].copy_from_slice(&index.to_le_bytes());
            assert_eq!(
                super::purchase_business_category(&business, &[0x1234, -32767]).unwrap(),
                expected
            );
        }
        business[8..10].copy_from_slice(&(-2_i16).to_le_bytes());
        assert!(super::purchase_business_category(&business, &[0x1234, -32767]).is_err());
    }
    #[test]
    #[ignore = "requires original purchase score observations"]
    fn matches_native_purchase_candidate() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_CANDIDATE")).unwrap();
        assert_eq!(&data[..8], b"MTPCAND1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let double = |at| f64::from_le_bytes(data[at..at + 8].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let commodity = word(at) as usize;
            let building_attribute = word(at + 4) as i8;
            let weights = std::array::from_fn(|i| f32::from_bits(word(at + 8 + i * 4)));
            let prices = [double(at + 20), double(at + 28)];
            let factor = f32::from_bits(word(at + 36));
            at += 40;
            let resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let business: [u8; 544] = data[at..at + 544].try_into().unwrap();
            at += 544;
            let account =
                super::SpendingAccount::from_bytes(data[at..at + 144].try_into().unwrap());
            at += 144;
            let expected_ratio = word(at);
            let expected_factor = word(at + 4);
            let expected_score = double(at + 8);
            at += 16;
            let count = word(at) as usize;
            at += 4;
            let expected_calls: Vec<_> = (0..count).map(|i| word(at + i * 4)).collect();
            at += count * 4;
            let calls = std::cell::RefCell::new(Vec::new());
            let mut price_index = 0;
            let result = resident
                .score_purchase_candidate(
                    &business,
                    &[account],
                    super::PurchaseCandidateInputs {
                        commodity,
                        value: 1.,
                        markup: 1.,
                        building_attribute,
                        weights,
                    },
                    |b, c| {
                        assert_eq!(b, &business);
                        assert_eq!(c, commodity);
                        calls.borrow_mut().push(0x462960);
                        let value = prices[price_index];
                        price_index += 1;
                        Ok(value)
                    },
                    |b| {
                        assert_eq!(b, &business);
                        calls.borrow_mut().push(0x462f30);
                        Ok(factor)
                    },
                )
                .unwrap();
            assert_eq!(calls.into_inner(), expected_calls, "case{case}");
            for (actual, expected) in [
                (result.ratio, expected_ratio),
                (result.category_factor, expected_factor),
            ] {
                let bits = actual.map_or(0xa5a5a5a5, f32::to_bits);
                assert!(
                    bits == expected
                        || (f32::from_bits(bits).is_nan() && f32::from_bits(expected).is_nan()),
                    "case{case} output"
                );
            }
            assert!(
                result.value.to_bits() == expected_score.to_bits()
                    || (result.value.is_nan() && expected_score.is_nan()),
                "case{case}: {} != {expected_score}",
                result.value
            );
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original purchase score observations"]
    fn matches_native_purchase_score() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_SCORE")).unwrap();
        assert_eq!(&data[..8], b"MTPSCOR1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let double = |at| f64::from_le_bytes(data[at..at + 8].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let chain = word(at) != 0;
            let category = word(at + 4) as usize;
            let account_mask = word(at + 12);
            let category_mask = word(at + 20);
            let expected_adjustments = word(at + 24);
            at += 28;
            let masks: [Option<u32>; 4] =
                std::array::from_fn(|i| (word(at + i * 8) != 0).then(|| word(at + i * 8 + 4)));
            at += 32;
            let scores: [f64; 128] = std::array::from_fn(|i| double(at + i * 8));
            at += 1024;
            let adjustment = double(at);
            at += 8;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let expected_score = double(at);
            let expected_choice = word(at + 8) != 0;
            at += 12;
            let count = word(at) as usize;
            at += 4;
            let expected_calls: Vec<_> = (0..count)
                .map(|i| (word(at + i * 8) as usize, word(at + i * 8 + 4) as usize))
                .collect();
            at += count * 8;
            let mut account = super::SpendingAccount { bytes: [0; 144] };
            account.bytes[0x84..0x88].copy_from_slice(&account_mask.to_le_bytes());
            let accounts = [account];
            let mut calls = Vec::new();
            let mut adjustments = 0;
            let mut scorer = |r: &mut super::Resident, cat| {
                r.score_purchase_category(
                    cat,
                    &accounts,
                    if cat == category { category_mask } else { 0 },
                    &masks,
                    |_, candidate, commodity| {
                        calls.push((candidate, commodity));
                        Ok(scores[candidate * 32 + commodity])
                    },
                    |_, cat| {
                        assert_eq!(cat, category);
                        adjustments += 1;
                        Ok(adjustment)
                    },
                )
            };
            if chain {
                assert_eq!(
                    resident
                        .select_purchase_category(&[None; 52], &mut [], &mut scorer)
                        .unwrap(),
                    expected_choice,
                    "case{case}"
                );
            } else {
                assert_eq!(
                    scorer(&mut resident, category).unwrap().to_bits(),
                    expected_score.to_bits(),
                    "case{case}"
                );
            }
            assert_eq!(resident.bytes.as_slice(), expected, "case{case}");
            assert_eq!(calls, expected_calls, "case{case}");
            assert_eq!(adjustments, expected_adjustments, "case{case}");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original category selection observations"]
    fn matches_native_purchase_category() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_CATEGORY")).unwrap();
        assert_eq!(&data[..8], b"MTCHOIC1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let chain = word(at) != 0;
            let linked = word(at + 4) != 0;
            at += 8;
            let scores: [f64; 52] = std::array::from_fn(|i| {
                f64::from_le_bytes(data[at + i * 8..at + i * 8 + 8].try_into().unwrap())
            });
            at += 416;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut player: [u8; 768] = data[at..at + 768].try_into().unwrap();
            at += 768;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let expected_resident = &data[at..at + 330];
            at += 330;
            let expected_player = &data[at..at + 768];
            at += 768;
            let expected_aux = &data[at..at + 284];
            at += 284;
            let result = word(at) != 0;
            at += 4;
            let owner = crate::economy::PlayerAccount::from_saved(&crate::save::SavedPlayer {
                metadata: &[0; 48],
                bytes: player[4..4 + 0x1b8].try_into().unwrap(),
            })
            .unwrap();
            let mut owners = [owner];
            let links = [linked.then_some(0); 52];
            let mut seen = 0;
            let mut score = |_: &mut super::Resident, category| {
                assert_eq!(category, seen);
                seen += 1;
                Ok(scores[category])
            };
            if chain {
                resident
                    .tick_with_purchase_actions(
                        &mut auxiliary,
                        &[[0.; 2]; 52],
                        |event, _, _| panic!("unexpected {event:?}"),
                        |helper, resident, _| {
                            assert_eq!(helper, super::ActionHelper::At45abd0);
                            resident.select_purchase_category(&links, &mut owners, &mut score)
                        },
                    )
                    .unwrap();
            } else {
                assert_eq!(
                    resident
                        .select_purchase_category(&links, &mut owners, &mut score)
                        .unwrap(),
                    result,
                    "case{case} return"
                );
            }
            assert_eq!(seen, 52);
            player[0x1ac..0x1b0].copy_from_slice(&owners[0].retail_counters[0].to_le_bytes());
            assert_eq!(
                resident.bytes.as_slice(),
                expected_resident,
                "case{case} resident"
            );
            assert_eq!(player.as_slice(), expected_player, "case{case} player");
            assert_eq!(auxiliary.as_slice(), expected_aux, "case{case} auxiliary");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original buyer/seller transaction observations"]
    fn matches_native_resident_transaction() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_TRANSACTION")).unwrap();
        assert_eq!(&data[..8], b"MTBUYER1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let through_tick = word(at) != 0;
            at += 4;
            let full = word(at) != 0;
            let resolved = word(at + 4) != 0;
            let inputs = super::RetailInputs {
                slot: word(at + 8) as usize,
                preference_limit: f32::from_bits(word(at + 12)),
                interaction_delay: f32::from_bits(word(at + 16)),
                unit_cost: f32::from_bits(word(at + 20)),
                tick: word(at + 24),
            };
            at += 28;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut business: [u8; 544] = data[at..at + 544].try_into().unwrap();
            at += 544;
            let mut account =
                super::SpendingAccount::from_bytes(data[at..at + 144].try_into().unwrap());
            at += 144;
            let expected_resident = &data[at..at + 330];
            at += 330;
            let expected_business = &data[at..at + 544];
            at += 544;
            let expected_account = &data[at..at + 144];
            at += 144;
            let count = word(at);
            at += 4;
            let expected_call = if count == 0 {
                None
            } else {
                assert_eq!(count, 1);
                let c = (word(at), word(at + 4) as i32);
                at += 8;
                Some(c)
            };
            let commodity = u32::from_le_bytes(resident.bytes[0x64..0x68].try_into().unwrap());
            let actual = if through_tick {
                let mut quantity = None;
                resident
                    .tick_with_purchase_actions(
                        &mut [0; 284],
                        &[[0.; 2]; 52],
                        |event, _, _| panic!("unexpected {event:?}"),
                        |helper, resident, _| match helper {
                            super::ActionHelper::At45e680 => Ok(true),
                            super::ActionHelper::At45f660 => {
                                if resolved {
                                    quantity = resident.purchase_resolved(
                                        &mut business,
                                        &mut account,
                                        inputs,
                                        None,
                                    )?;
                                }
                                Ok(false)
                            }
                            _ => panic!("unexpected {helper:?}"),
                        },
                    )
                    .unwrap();
                quantity
            } else if !resolved {
                None
            } else if full {
                resident
                    .purchase_resolved(&mut business, &mut account, inputs, None)
                    .unwrap()
            } else {
                let mut calls = 0;
                let q = resident
                    .purchase_with_seller(&mut business, &mut account, inputs, |_, id, q| {
                        calls += 1;
                        assert_eq!(Some((id, q)), expected_call, "case{case} call");
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(calls, count);
                q
            };
            assert_eq!(
                actual.map(|q| (commodity, q)),
                expected_call,
                "case{case} outcome"
            );
            assert_eq!(
                resident.bytes.as_slice(),
                expected_resident,
                "case{case} resident"
            );
            assert_eq!(
                business.as_slice(),
                expected_business,
                "case{case} business"
            );
            assert_eq!(
                account.bytes.as_slice(),
                expected_account,
                "case{case} account"
            );
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original purchase action observations"]
    fn matches_native_purchase_action() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_PURCHASE_ACTION")).unwrap();
        assert_eq!(&data[..8], b"MTPUR001");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let through_tick = word(at) != 0;
            let mutation = (word(at + 4), word(at + 8));
            at += 12;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let expected = &data[at..at + 330];
            at += 330;
            let expected_aux = &data[at..at + 284];
            at += 284;
            let result = word(at);
            let count = word(at + 4) as usize;
            at += 8;
            let mut calls = Vec::new();
            for _ in 0..count {
                calls.push((word(at), word(at + 4)));
                at += 8;
            }
            let mut seen = 0;
            let mut callback =
                |helper: super::ActionHelper, resident: &mut super::Resident, _: &mut [u8; 284]| {
                    let address = helper as u32;
                    assert_eq!(address, calls[seen].0, "case{case} callback");
                    let done = calls[seen].1 != 0;
                    seen += 1;
                    if mutation.0 == address {
                        resident.bytes[0xd0..0xd4].copy_from_slice(&mutation.1.to_le_bytes());
                    }
                    Ok(done)
                };
            if through_tick {
                resident
                    .tick_with_purchase_actions(
                        &mut auxiliary,
                        &[[0.; 2]; 52],
                        |event, _, _| panic!("unexpected {event:?}"),
                        &mut callback,
                    )
                    .unwrap();
            } else {
                assert_eq!(
                    resident
                        .advance_purchase_action(&mut auxiliary, &mut callback)
                        .unwrap(),
                    result != 0,
                    "case{case} return"
                );
            }
            assert_eq!(seen, count, "case{case} calls");
            assert_eq!(resident.bytes.as_slice(), expected, "case{case} resident");
            assert_eq!(auxiliary.as_slice(), expected_aux, "case{case} auxiliary");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original home-action observations"]
    fn matches_native_home_action() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_HOME")).unwrap();
        assert_eq!(&data[..8], b"MTHOME01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x037f);
        let mut at = 16;
        for case in 0..word(8) {
            let mut rng = [word(at), word(at + 4), word(at + 8)];
            let expected_rng = [word(at + 12), word(at + 16), word(at + 20)];
            at += 24;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let expected_return = word(at) != 0;
            let count = word(at + 4) as usize;
            at += 8;
            let seen = std::cell::Cell::new(0);
            let call = |address| {
                let i = seen.get();
                assert!(i < count, "extra call case{case}");
                assert_eq!(word(at + i * 16), address, "call case{case}");
                seen.set(i + 1);
                i
            };
            let returned = resident
                .advance_home_action(
                    &mut [0; 284],
                    &mut rng,
                    |helper, _, _| {
                        let i = call(helper as u32);
                        Ok(word(at + i * 16 + 4) != 0)
                    },
                    |index, value| {
                        let i = call(0x48ded0);
                        assert_eq!(index, word(at + i * 16 + 8), "arrival index case{case}");
                        assert_eq!(
                            value.to_bits(),
                            word(at + i * 16 + 12),
                            "arrival value case{case}"
                        );
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(returned, expected_return, "return case{case}");
            assert_eq!(seen.get(), count, "call count case{case}");
            assert_eq!(resident.bytes.as_slice(), expected, "resident case{case}");
            assert_eq!(rng, expected_rng, "RNG case{case}");
            at += count * 16;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original local roaming observations"]
    fn matches_native_local_roaming() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_ROAMING")).unwrap();
        assert_eq!(&data[..8], b"MTROBIN1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x037f);
        let mut at = 16;
        for case in 0..word(8) {
            let centers = std::array::from_fn(|i| {
                [
                    f32::from_bits(word(at + i * 8)),
                    f32::from_bits(word(at + i * 8 + 4)),
                ]
            });
            at += 416;
            let mut counts = std::array::from_fn(|i| word(at + i * 4));
            at += 208;
            let expected_counts: [u32; 52] = std::array::from_fn(|i| word(at + i * 4));
            at += 208;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let count = word(at) as usize;
            at += 4;
            let seam_calls: Vec<_> = (0..count)
                .filter_map(|i| {
                    let address = word(at + i * 8);
                    matches!(address, 0x45ce10 | 0x45d900)
                        .then_some((address, word(at + i * 8 + 4)))
                })
                .collect();
            let seen = std::cell::Cell::new(0);
            let call = |address| {
                let index = seen.get();
                assert!(index < seam_calls.len(), "extra callback case{case}");
                assert_eq!(seam_calls[index].0, address, "call case{case}");
                seen.set(index + 1);
                seam_calls[index].1 != 0
            };
            resident
                .advance_local_roaming(
                    &centers,
                    &mut counts,
                    |_| {
                        call(0x45ce10);
                        Ok(())
                    },
                    |_| Ok(call(0x45d900)),
                )
                .unwrap();
            assert_eq!(seen.get(), seam_calls.len(), "calls case{case}");
            assert_eq!(counts, expected_counts, "category counts case{case}");
            assert_eq!(resident.bytes.as_slice(), expected, "resident case{case}");
            at += count * 8;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original resident action12 observations"]
    fn matches_native_roaming_action() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_ACTION12")).unwrap();
        assert_eq!(&data[..8], b"MTA12BIN");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(16), word(20)), (1, 844, 3));
        let mut at = 24;
        for case in 0..word(12) {
            let calls = word(at + 20) as usize;
            at += 32;
            let mut rng = [word(at), word(at + 4), word(at + 8)];
            let expected_rng = [word(at + 12), word(at + 16), word(at + 20)];
            at += 24;
            let enabled: [u8; 52] = data[at..at + 52].try_into().unwrap();
            assert_eq!(&enabled, &data[at + 52..at + 104]);
            at += 104;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let mut seen = 0;
            let mut auxiliary = [0x59; 284];
            let result = resident
                .advance_roaming_action(&mut auxiliary, &enabled, &mut rng, |helper, _, _| {
                    assert!(seen < calls, "extra call case{case}");
                    assert_eq!(helper as u32, word(at + seen * 8), "call case{case}");
                    let result = word(at + seen * 8 + 4) != 0;
                    seen += 1;
                    Ok(result)
                })
                .unwrap();
            assert!(!result);
            assert_eq!(seen, calls);
            assert_eq!(rng, expected_rng, "RNG case{case}");
            assert_eq!(resident.bytes.as_slice(), expected, "resident case{case}");
            assert_eq!(auxiliary, [0x59; 284]);
            at += 24;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original housing action observations"]
    fn matches_native_housing_action_transitions() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_HOUSING_ACTION")).unwrap();
        assert_eq!(&data[..8], b"MTHACT01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x037f);
        let mut at = 16;
        let mut compared = 0;
        for case in 0..word(8) {
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let expected = &data[at..at + 330];
            at += 330;
            let returned = word(at) != 0;
            let count = word(at + 4) as usize;
            at += 8;
            let mut calls = Vec::new();
            for _ in 0..count {
                calls.push((word(at), word(at + 4), word(at + 8)));
                at += 12;
            }
            // Successful assignment has deeper list/address/schedule seams;
            // this oracle covers dispatcher writes, not those seam bodies.
            if returned {
                continue;
            }
            let mut seen = 0;
            let result = resident
                .advance_housing_action(|action, _| {
                    let address = match action {
                        super::HousingAction::Choose => 0x45a470,
                        super::HousingAction::TravelA => 0x45ecb0,
                        super::HousingAction::TravelB => 0x45e830,
                        super::HousingAction::Assign => 0x45a640,
                    };
                    assert_eq!(calls[seen].0, address, "case{case}");
                    let result = calls[seen].1 != 0;
                    seen += 1;
                    Ok(result)
                })
                .unwrap();
            assert_eq!(result, returned, "case{case} result");
            assert_eq!(seen, count, "case{case} calls");
            assert_eq!(resident.bytes.as_slice(), expected, "case{case} resident");
            compared += 1;
        }
        assert_eq!(compared, 19);
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original resident action observations"]
    fn matches_native_resident_action() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_ACTION")).unwrap();
        assert_eq!(&data[..8], b"MTACT001");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let through_tick = word(at) != 0;
            at += 4;
            let mutation = word(at);
            at += 4;
            let mut resident = super::Resident::from_bytes(data[at..at + 330].try_into().unwrap());
            at += 330;
            let mut auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let expected = &data[at..at + 330];
            at += 330;
            let expected_aux = &data[at..at + 284];
            at += 284;
            let count = word(at) as usize;
            at += 4;
            let mut calls = Vec::new();
            for _ in 0..count {
                calls.push((word(at), word(at + 4)));
                at += 8;
            }
            let mut seen = 0;
            let mut callback =
                |helper: super::ActionHelper, resident: &mut super::Resident, _: &mut [u8; 284]| {
                    assert_eq!(helper as u32, calls[seen].0, "case{case} callback");
                    let result = calls[seen].1 != 0;
                    seen += 1;
                    if mutation != u32::MAX {
                        resident.bytes[0xcc..0xd0].copy_from_slice(&mutation.to_le_bytes());
                    }
                    Ok(result)
                };
            if through_tick {
                resident
                    .tick_with_actions(
                        &mut auxiliary,
                        &[[0.; 2]; 52],
                        |event, _, _| panic!("unexpected {event:?}"),
                        &mut callback,
                    )
                    .unwrap();
            } else {
                resident
                    .advance_action(&mut auxiliary, &mut callback)
                    .unwrap();
            }
            assert_eq!(seen, count, "case{case} calls");
            assert_eq!(resident.bytes.as_slice(), expected, "case{case} resident");
            assert_eq!(auxiliary.as_slice(), expected_aux, "case{case} auxiliary");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original resident tick observations"]
    fn matches_native_resident_tick() {
        check_native_resident_tick(false);
    }

    #[test]
    #[ignore = "requires original population loop and count-mutation observations"]
    fn matches_native_population_tick() {
        check_native_resident_tick(true);
    }

    fn check_native_resident_tick(population: bool) {
        let data = std::fs::read(crate::fixtures::path(if population {
            "OPENMT2001_NATIVE_POPULATION_TICK"
        } else {
            "OPENMT2001_NATIVE_RESIDENT_TICK"
        }))
        .unwrap();
        assert_eq!(
            &data[..8],
            if population { b"MTPRE002" } else { b"MTPRE001" }
        );
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        let centers = std::array::from_fn(|i| [i as f32 * 3.25, i as f32 * -7.5]);
        for case in 0..word(8) {
            let mut mutations = Vec::new();
            let counts = population.then(|| {
                let counts = (word(at), word(at + 4));
                let changes = word(at + 8) as usize;
                at += 12;
                for _ in 0..changes {
                    mutations.push((word(at) as usize, word(at + 4)));
                    at += 8;
                }
                counts
            });
            let count = word(at) as usize;
            at += 4;
            let mut records = Vec::new();
            for _ in 0..count {
                let resident = super::Resident::from_pool_bytes(
                    data[at..at + 330].try_into().unwrap(),
                    data[at + 330],
                );
                at += 331;
                let auxiliary: [u8; 284] = data[at..at + 284].try_into().unwrap();
                at += 284;
                let expected = &data[at..at + 330];
                at += 330;
                let expected_aux = &data[at..at + 284];
                at += 284;
                records.push((resident, auxiliary, expected, expected_aux));
            }
            let calls = word(at) as usize;
            at += 4;
            let mut expected_calls = Vec::new();
            let mut movement_done = true;
            for _ in 0..calls {
                expected_calls.push((word(at), word(at + 4)));
                movement_done = word(at + 8) != 0;
                at += 12;
            }
            let expected_visits = if population {
                let count = word(at) as usize;
                at += 4;
                let visits: Vec<_> = (0..count).map(|i| word(at + i * 4) as usize).collect();
                at += count * 4;
                visits
            } else {
                (0..count).collect()
            };
            let mut actual_calls = Vec::new();
            let mut actual_visits = Vec::new();
            let mut residents = crate::economy::ResidentAccounts {
                active_count: counts.map_or(count as u32, |c| c.0),
                nodes: records.iter().map(|r| r.0.clone()).collect(),
                accounts: vec![],
            };
            residents
                .advance_population(|slot, residents| {
                    actual_visits.push(slot);
                    if let Some((_, count)) = mutations.iter().find(|m| m.0 == slot) {
                        residents.active_count = *count;
                    }
                    residents.tick_resident(
                        slot,
                        &mut records[slot].1,
                        &centers,
                        |event, _, _, _| {
                            let address = match event {
                                super::TickEvent::Movement => 0x45b850,
                                super::TickEvent::RefreshIntent => 0x45fc90,
                                super::TickEvent::Action => 0x460090,
                            };
                            actual_calls.push((address, slot as u32));
                            Ok(movement_done)
                        },
                    )
                })
                .unwrap();
            assert_eq!(actual_visits, expected_visits, "case{case} visitation");
            assert_eq!(
                residents.active_count,
                counts.map_or(count as u32, |c| c.1),
                "case{case} count"
            );
            for (slot, (resident, (_, auxiliary, expected, expected_aux))) in
                residents.nodes.iter().zip(records).enumerate()
            {
                assert_eq!(resident.bytes.as_slice(), expected, "case{case} slot{slot}");
                assert_eq!(
                    auxiliary.as_slice(),
                    expected_aux,
                    "case{case} slot{slot} auxiliary"
                );
            }
            assert_eq!(actual_calls, expected_calls, "case{case} calls");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires complete original resident creation observations"]
    fn matches_native_resident_creation() {
        check_native_resident_creation(false);
    }

    #[test]
    #[ignore = "requires full original saved-resident loader observations"]
    fn matches_native_resident_load() {
        check_native_resident_creation(true);
    }

    fn check_native_resident_creation(load: bool) {
        use crate::economy::{
            ResidentAccounts, ResidentCreationError, ResidentCreationInputs, ResidentCreationState,
            ResidentModelBinding,
        };
        let bytes = std::fs::read(crate::fixtures::path(if load {
            "OPENMT2001_NATIVE_RESIDENT_LOAD"
        } else {
            "OPENMT2001_NATIVE_RESIDENT_CREATION"
        }))
        .unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], if load { b"MTLOADR1" } else { b"MTCREATE" });
        assert_eq!(word(8), if load { 4 } else { 8 });
        let source = std::fs::read_to_string(crate::fixtures::path("DEFAULT_SETTINGS")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let preferences = super::Preferences::from_v14_defaults(&source, &image).unwrap();
        let groups = super::PreferenceGroups::from_v14_defaults(&source).unwrap();
        let parameters = std::fs::read_to_string(crate::fixtures::path("PARAMETERS")).unwrap();
        let movement = super::MovementSettings::from_parameters(&parameters, &image).unwrap();
        assert_eq!(
            movement.speed_by_age.map(f32::to_bits),
            std::array::from_fn(|i| word(12 + i * 4))
        );
        assert_eq!(movement.variation.to_bits(), word(32));
        let saved_bytes = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&saved_bytes).unwrap();
        let commodities = save.economy().unwrap().resident_commodities();
        let objectives = super::DailyObjectives {
            rows: std::array::from_fn(|i| {
                (0..3).map(|j| word(36 + (i * 3 + j) * 4) as i32).collect()
            }),
        };
        let timetables = super::Timetables {
            rows: std::array::from_fn(|i| {
                std::array::from_fn(|j| word(108 + (i * 24 + j) * 4) as i32)
            }),
        };
        let commodity_masks = std::array::from_fn(|i| word(9708 + i * 4));
        let models = std::array::from_fn(|i| ResidentModelBinding {
            reference: word(9728 + i * 120),
            definition: bytes[9732 + i * 120..9848 + i * 120].try_into().unwrap(),
        });
        let mut at = 14192;
        for case in 0..word(8) {
            let mut categories = std::array::from_fn(|i| crate::housing::Category {
                buildings: vec![],
                value: 0.,
                center: Some([
                    f32::from_bits(word(13568 + i * 12)),
                    f32::from_bits(word(13572 + i * 12)),
                ]),
                resident_counter: word(13576 + i * 12),
            });
            let requested = [word(at) as i32, word(at + 4) as i32, word(at + 8) as i32];
            let hour = word(at + 12) as usize;
            let initialization_rng = super::InitializationRng {
                constructor: word(at + 16),
                movement: word(at + 20),
                objective: word(at + 24),
            };
            let identity_rng = word(at + 28);
            let preference_rng = word(at + 32);
            let active_count = word(at + 36);
            let capacity = word(at + 40) as usize;
            let used = word(at + 44) as usize;
            let countdown_period = word(at + 48) as i32;
            at += 52;
            let nodes = bytes[at..at + 2000 * 0x150]
                .chunks_exact(0x150)
                .map(|r| {
                    super::Resident::from_runtime_bytes(
                        r[..0x14a].try_into().unwrap(),
                        r[0x14a],
                        if load {
                            u32::from_le_bytes(r[0x14c..0x150].try_into().unwrap())
                        } else {
                            0
                        },
                    )
                })
                .collect();
            at += 2000 * 0x150;
            let accounts = bytes[at..at + 2000 * 0x90]
                .chunks_exact(0x90)
                .map(|r| super::SpendingAccount::from_bytes(r.try_into().unwrap()))
                .collect();
            at += 2000 * 0x90;
            let slots = bytes[at..at + capacity * 0x11c]
                .chunks_exact(0x11c)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += capacity * 0x11c;
            let template = bytes[at..at + 0x11c].try_into().unwrap();
            at += 0x11c;
            let mut residents = ResidentAccounts {
                nodes,
                accounts,
                active_count,
            };
            let mut state = ResidentCreationState {
                auxiliary: super::AuxiliaryPool::from_slots(slots, used).unwrap(),
                initialization_rng,
                identity_rng,
                preference_rng,
            };
            let personal = super::InitializationInputs {
                movement: &movement,
                preferences: &preferences,
                objectives: &objectives,
                timetables: &timetables,
                commodities: &commodities,
                commodity_masks,
                hour,
            };
            let inputs = ResidentCreationInputs {
                personal: &personal,
                preference_groups: &groups,
                models: &models,
                countdown_period,
            };
            let result = if load {
                let saved = bytes[at..at + 0x14a].try_into().unwrap();
                at += 0x14a;
                let centers = std::array::from_fn(|i| categories[i].center.unwrap());
                residents
                    .load_resident(
                        saved,
                        template,
                        &inputs,
                        &mut state,
                        &mut categories,
                        &centers,
                    )
                    .map(Some)
            } else {
                residents.create_resident(requested, template, &inputs, &mut state, &mut categories)
            };
            let actual = match result {
                Ok(Some(index)) => (0, index as u32),
                Ok(None) => (1, u32::MAX),
                Err(ResidentCreationError::AuxiliaryExhausted { resident }) => (2, resident as u32),
                Err(ResidentCreationError::AccountsExhausted { resident }) => (3, resident as u32),
                Err(error) => panic!("case {case}: {error:?}"),
            };
            assert_eq!(actual, (word(at), word(at + 4)), "case {case} outcome");
            assert_eq!(
                residents.active_count,
                word(at + 8),
                "case {case} active count"
            );
            assert_eq!(
                state.auxiliary.used(),
                word(at + 12) as usize,
                "case {case} aux used"
            );
            at += 16;
            assert_eq!(
                [
                    state.initialization_rng.constructor,
                    state.initialization_rng.movement,
                    state.initialization_rng.objective,
                    state.identity_rng,
                    state.preference_rng
                ],
                std::array::from_fn(|i| word(at + i * 4)),
                "case {case} RNG"
            );
            at += 20;
            for (i, category) in categories.iter().enumerate() {
                assert_eq!(
                    category.resident_counter,
                    word(at + i * 4),
                    "case {case} category {i}"
                );
            }
            at += 52 * 4;
            for (i, node) in residents.nodes.iter().enumerate() {
                let start = at + i * 0x150;
                if load {
                    assert_eq!(
                        node.auxiliary_reference(),
                        word(start + 0x14c),
                        "case {case} node {i} auxiliary reference"
                    );
                }
                for (offset, (&actual, &expected)) in node
                    .as_bytes()
                    .iter()
                    .zip(&bytes[start..start + 0x14a])
                    .enumerate()
                {
                    assert_eq!(actual, expected, "case {case} node {i} offset {offset:x}");
                }
                assert_eq!(
                    node.active_byte(),
                    bytes[start + 0x14a],
                    "case {case} node {i} active"
                );
            }
            at += 2000 * 0x150;
            for (i, account) in residents.accounts.iter().enumerate() {
                let start = at + i * 0x90;
                assert_eq!(
                    account.as_bytes().as_slice(),
                    &bytes[start..start + 0x90],
                    "case {case} account {i}"
                );
            }
            at += 2000 * 0x90;
            for (i, slot) in state.auxiliary.slots().iter().enumerate() {
                let start = at + i * 0x11c;
                assert_eq!(
                    slot.as_slice(),
                    &bytes[start..start + 0x11c],
                    "case {case} auxiliary {i}"
                );
            }
            at += capacity * 0x11c;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident model selection/binding observations"]
    fn matches_native_resident_model() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_MODEL")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRMODEL");
        assert_eq!((word(8), word(12)), (10000, 9));
        let mut at = 16;
        for case in 0..word(8) {
            let mut raw = [0; 0x14a];
            raw[..4].copy_from_slice(&word(at).to_le_bytes());
            raw[0x148..0x14a].copy_from_slice(&(word(at + 4) as u16).to_le_bytes());
            let resident = super::Resident::from_bytes(raw);
            assert_eq!(
                u32::from(resident.model_variant()),
                word(at + 8),
                "model selector {case}"
            );
            at += 12;
        }
        for case in 0..word(12) {
            let reference = word(at);
            at += 4;
            let definition: [u8; 0x74] = bytes[at..at + 0x74].try_into().unwrap();
            at += 0x74;
            let mut auxiliary: [u8; 0x11c] = bytes[at..at + 0x11c].try_into().unwrap();
            at += 0x11c;
            super::bind_auxiliary_model(&mut auxiliary, reference, &definition);
            assert_eq!(
                auxiliary.as_slice(),
                &bytes[at..at + 0x11c],
                "model binding {case}"
            );
            at += 0x11c;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original auxiliary pool allocation observations"]
    fn matches_native_resident_auxiliary_pool() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_AUXILIARY_POOL")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAUXPOL");
        assert_eq!(word(8), 6);
        let mut at = 12;
        for case in 0..word(8) {
            let capacity = word(at) as usize;
            let used = word(at + 4) as usize;
            let calls = word(at + 8);
            at += 12;
            let slots = bytes[at..at + capacity * 0x11c]
                .chunks_exact(0x11c)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += capacity * 0x11c;
            let mut pool = super::AuxiliaryPool::from_slots(slots, used).unwrap();
            for call in 0..calls {
                let template: [u8; 0x11c] = bytes[at..at + 0x11c].try_into().unwrap();
                at += 0x11c;
                let selected = pool.allocate(&template).map_or(u32::MAX, |i| i as u32);
                assert_eq!(selected, word(at), "case {case} call {call} index");
                assert_eq!(
                    pool.used(),
                    word(at + 4) as usize,
                    "case {case} call {call} count"
                );
                at += 8;
                for (i, slot) in pool.slots().iter().enumerate() {
                    let start = at + i * 0x11c;
                    assert_eq!(
                        slot.as_slice(),
                        &bytes[start..start + 0x11c],
                        "case {case} call {call} slot {i}"
                    );
                }
                at += capacity * 0x11c;
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident auxiliary template observations"]
    fn matches_native_resident_auxiliary() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_AUXILIARY")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRAUX01");
        assert_eq!(word(8), 15);
        let mut at = 12;
        for case in 0..word(8) {
            let pool_index = word(at) as u16;
            at += 4;
            let mut template: [u8; 0x11c] = bytes[at..at + 0x11c].try_into().unwrap();
            at += 0x11c;
            super::initialize_auxiliary_template(&mut template, pool_index);
            for (offset, (&actual, &expected)) in
                template.iter().zip(&bytes[at..at + 0x11c]).enumerate()
            {
                assert_eq!(actual, expected, "case {case} auxiliary offset {offset:x}");
            }
            at += 0x11c;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original complete resident constructor observations"]
    fn matches_native_resident_constructor() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_CONSTRUCTOR")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRCTOR2");
        assert_eq!(word(8), 60);
        let source = std::fs::read_to_string(crate::fixtures::path("DEFAULT_SETTINGS")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let preferences = super::Preferences::from_v14_defaults(&source, &image).unwrap();
        let saved_bytes = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&saved_bytes).unwrap();
        let commodities = save.economy().unwrap().resident_commodities();
        let parameters = std::fs::read_to_string(crate::fixtures::path("PARAMETERS")).unwrap();
        let movement = super::MovementSettings::from_parameters(&parameters, &image).unwrap();
        assert_eq!(
            movement.speed_by_age.map(f32::to_bits),
            std::array::from_fn(|i| word(12 + i * 4))
        );
        assert_eq!(movement.variation.to_bits(), word(32));
        let objectives = super::DailyObjectives {
            rows: std::array::from_fn(|i| {
                (0..3).map(|j| word(36 + (i * 3 + j) * 4) as i32).collect()
            }),
        };
        let timetables = super::Timetables {
            rows: std::array::from_fn(|i| {
                std::array::from_fn(|j| word(108 + (i * 24 + j) * 4) as i32)
            }),
        };
        let commodity_masks = std::array::from_fn(|i| word(9708 + i * 4));
        let mut at = 9728;
        for case in 0..word(8) {
            let identity = [word(at), word(at + 4), word(at + 8)];
            let hour = word(at + 12) as usize;
            let mut rng = super::InitializationRng {
                constructor: word(at + 16),
                movement: word(at + 20),
                objective: word(at + 24),
            };
            at += 28;
            let mut resident =
                super::Resident::from_bytes(bytes[at..at + 0x14a].try_into().unwrap());
            at += 0x14a;
            let mut account =
                super::SpendingAccount::from_bytes(bytes[at..at + 0x90].try_into().unwrap());
            at += 0x90;
            let inputs = super::InitializationInputs {
                movement: &movement,
                preferences: &preferences,
                objectives: &objectives,
                timetables: &timetables,
                commodities: &commodities,
                commodity_masks,
                hour,
            };
            resident
                .initialize_personal(identity, &mut account, &inputs, &mut rng)
                .unwrap();
            for (offset, (&actual, &expected)) in resident
                .as_bytes()
                .iter()
                .zip(&bytes[at..at + 0x14a])
                .enumerate()
            {
                assert_eq!(actual, expected, "case {case} resident offset {offset:x}");
            }
            at += 0x14a;
            assert_eq!(
                account.as_bytes().as_slice(),
                &bytes[at..at + 0x90],
                "case {case} account"
            );
            at += 0x90;
            assert_eq!(
                [rng.constructor, rng.movement, rng.objective],
                [word(at), word(at + 4), word(at + 8)],
                "case {case} RNG"
            );
            at += 12;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident personal-field observations"]
    fn matches_native_resident_personal() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_PERSONAL")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRPERS1");
        assert_eq!(word(8), 60);
        let mut at = 12;
        for case in 0..word(8) {
            let mut rng = word(at);
            let age = word(at + 4);
            let class = word(at + 8);
            at += 12;
            let mut resident =
                super::Resident::from_bytes(bytes[at..at + 0x14a].try_into().unwrap());
            at += 0x14a;
            resident
                .initialize_personal_fields(age, class, &mut rng)
                .unwrap();
            for (offset, (&actual, &expected)) in resident
                .as_bytes()
                .iter()
                .zip(&bytes[at..at + 0x14a])
                .enumerate()
            {
                assert_eq!(actual, expected, "case {case} resident offset {offset:x}");
            }
            at += 0x14a;
            assert_eq!(rng, word(at), "RNG case {case}");
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident initial placement observations"]
    fn matches_native_resident_placement() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_PLACEMENT")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRPLACE");
        assert_eq!(word(8), 256);
        let mut at = 12;
        for case in 0..word(8) {
            let mut rng = word(at);
            let period = word(at + 4) as i32;
            let handle = word(at + 8);
            at += 12;
            let mut categories = std::array::from_fn(|i| crate::housing::Category {
                buildings: vec![],
                value: 0.,
                center: Some([
                    f32::from_bits(word(at + i * 12)),
                    f32::from_bits(word(at + i * 12 + 4)),
                ]),
                resident_counter: word(at + i * 12 + 8),
            });
            at += 52 * 12;
            let mut resident =
                super::Resident::from_bytes(bytes[at..at + 0x14a].try_into().unwrap());
            at += 0x14a;
            let mut auxiliary: [u8; 0x11c] = bytes[at..at + 0x11c].try_into().unwrap();
            at += 0x11c;
            let mut raw_categories = [[0xa5; 0x17c]; 52];
            for (row, category) in raw_categories.iter_mut().zip(&categories) {
                for (axis, value) in category.center.unwrap().into_iter().enumerate() {
                    row[0xe0 + axis * 4..0xe4 + axis * 4].copy_from_slice(&value.to_le_bytes());
                }
                row[0x88..0x8c].copy_from_slice(&category.resident_counter.to_le_bytes());
            }
            let mut raw_resident = super::Resident::from_bytes(*resident.as_bytes());
            let mut raw_auxiliary = auxiliary;
            let mut raw_rng = rng;
            raw_resident
                .initialize_runtime(
                    handle,
                    &mut raw_auxiliary,
                    &mut raw_rng,
                    period,
                    &mut raw_categories,
                )
                .unwrap();
            resident
                .initialize_runtime(handle, &mut auxiliary, &mut rng, period, &mut categories)
                .unwrap();
            assert_eq!(raw_resident.as_bytes(), resident.as_bytes());
            assert_eq!(raw_auxiliary, auxiliary);
            assert_eq!(raw_rng, rng);
            for (i, row) in raw_categories.iter().enumerate() {
                assert_eq!(
                    u32::from_le_bytes(row[0x88..0x8c].try_into().unwrap()),
                    categories[i].resident_counter
                );
                assert!(
                    row[..0x88]
                        .iter()
                        .chain(row[0x8c..0xe0].iter())
                        .chain(row[0xe8..].iter())
                        .all(|&b| b == 0xa5)
                );
            }
            for (offset, (&actual, &expected)) in resident
                .as_bytes()
                .iter()
                .zip(&bytes[at..at + 0x14a])
                .enumerate()
            {
                assert_eq!(actual, expected, "case {case} resident offset {offset:x}");
            }
            at += 0x14a;
            assert_eq!(
                auxiliary.as_slice(),
                &bytes[at..at + 0x11c],
                "auxiliary case {case}"
            );
            at += 0x11c;
            assert_eq!(rng, word(at), "RNG case {case}");
            at += 4;
            for (i, category) in categories.iter().enumerate() {
                assert_eq!(
                    category.resident_counter,
                    word(at + i * 4),
                    "case {case} category {i}"
                );
            }
            at += 52 * 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident runtime reset observations"]
    fn matches_native_resident_reset() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_RESET")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRRESET");
        assert_eq!(word(8), 12);
        let mut at = 12;
        for case in 0..12 {
            let mut rng = word(at);
            let period = word(at + 4) as i32;
            let handle = word(at + 8);
            at += 12;
            let mut resident =
                super::Resident::from_bytes(bytes[at..at + 0x14a].try_into().unwrap());
            at += 0x14a;
            let mut auxiliary: [u8; 0x11c] = bytes[at..at + 0x11c].try_into().unwrap();
            at += 0x11c;
            resident
                .reset_runtime_state(handle, &mut auxiliary, &mut rng, period)
                .unwrap();
            assert_eq!(
                resident.as_bytes().as_slice(),
                &bytes[at..at + 0x14a],
                "resident case {case}"
            );
            at += 0x14a;
            assert_eq!(
                auxiliary.as_slice(),
                &bytes[at..at + 0x11c],
                "auxiliary case {case}"
            );
            at += 0x11c;
            assert_eq!(rng, word(at), "RNG case {case}");
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident identity observations and DEFAULT"]
    fn matches_native_resident_identity() {
        let source = std::fs::read_to_string(crate::fixtures::path("DEFAULT_SETTINGS")).unwrap();
        let groups = super::PreferenceGroups::from_v14_defaults(&source).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_IDENTITY")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTIDENT1");
        assert_eq!((word(8), word(12)), (35, 120));
        let mut at = 16;
        for row in &groups.rows {
            assert_eq!(row.len(), usize::from(bytes[at + 32]));
            assert_eq!(
                row,
                &bytes[at..at + row.len()]
                    .iter()
                    .map(|&v| v as i8)
                    .collect::<Vec<_>>()
            );
            at += 33;
        }
        for case in 0..35 {
            let mut age_rng = word(at);
            let mut pref_rng = word(at + 4);
            let requested = std::array::from_fn(|i| word(at + 8 + i * 4) as i32);
            let expected: [i32; 3] = std::array::from_fn(|i| word(at + 20 + i * 4) as i32);
            assert_eq!(
                groups
                    .resolve(requested, &mut age_rng, &mut pref_rng)
                    .unwrap(),
                expected,
                "identity case {case}"
            );
            assert_eq!(
                (age_rng, pref_rng),
                (word(at + 32), word(at + 36)),
                "RNG case {case}"
            );
            at += 40;
        }
        assert_eq!(at, bytes.len());
    }

    use super::*;

    #[test]
    #[ignore = "requires original saves and complete native daily-loop observations"]
    fn matches_native_resident_daily_update() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let bytes = read("OPENMT2001_NATIVE_RESIDENT_DAILY");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTDAILY1");
        assert_eq!(
            (word(8), word(12), word(16), word(20)),
            (2, 2000, 0x14a, 0x027f)
        );
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let defaults = String::from_utf8(read("OPENMT2001_DEFAULT_SETTINGS")).unwrap();
        let preferences = Preferences::from_v14_defaults(&defaults, &image).unwrap();
        let welfare = Welfare::from_v14_defaults(&defaults).unwrap();
        let timetable = String::from_utf8(read("OPENMT2001_TIMETABLE")).unwrap();
        let objectives = DailyObjectives::from_timetable(&timetable).unwrap();
        let mut at = 24;
        for name in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let original = read(name);
            let save = crate::save::SaveFile::parse(&original).unwrap();
            let saved = save.residents().unwrap();
            let mut residents = crate::economy::ResidentAccounts::from_saved(&saved);
            let count = word(at + 32) as usize;
            assert_eq!(count, residents.nodes.len());
            let mut rng = word(at + 36);
            let expected_rng = word(at + 40);
            at += 44;
            assert_eq!(&bytes[at..at + 2000 * 0x90], saved.accounts);
            at += 2000 * 0x90;
            let expected_accounts = &bytes[at..at + 2000 * 0x90];
            at += 2000 * 0x90;
            for node in &residents.nodes {
                assert_eq!(node.as_bytes().as_slice(), &bytes[at..at + 0x14a]);
                at += 0x14a;
            }
            residents
                .update_daily(
                    &welfare,
                    &preferences,
                    &save.economy().unwrap().resident_commodities(),
                    &objectives,
                    &mut rng,
                )
                .unwrap();
            assert_eq!(rng, expected_rng, "{name} objective RNG");
            for (i, (account, expected)) in residents
                .accounts
                .iter()
                .zip(expected_accounts.chunks_exact(0x90))
                .enumerate()
            {
                assert_eq!(account.as_bytes().as_slice(), expected, "{name} account{i}");
            }
            for (i, node) in residents.nodes.iter().enumerate() {
                assert_eq!(
                    node.as_bytes().as_slice(),
                    &bytes[at..at + 0x14a],
                    "{name} resident{i}"
                );
                at += 0x14a;
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original timetable and native objective binding observations"]
    fn matches_native_daily_objective_rows() {
        let source = std::fs::read_to_string(crate::fixtures::path("TIMETABLE")).unwrap();
        let rows = DailyObjectives::from_timetable(&source).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_DAILY_OBJECTIVES")).unwrap();
        let word = |at| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTDOBJ01");
        assert_eq!((word(8), word(12), word(16)), (6, 33, 0x027f));
        assert_eq!(bytes.len(), 20 + 6 * 33 * 4);
        for selector in 0..6 {
            let row = rows.get(selector).unwrap();
            let at = 20 + selector * 33 * 4;
            assert_eq!(row.len(), usize::try_from(word(at + 128)).unwrap());
            for slot in 0..32 {
                assert_eq!(
                    row.get(slot).copied().unwrap_or(-1),
                    word(at + slot * 4),
                    "selector {selector}, slot {slot}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires original timetable and native timetable binding observations"]
    fn matches_native_timetable_rows() {
        let source = std::fs::read_to_string(crate::fixtures::path("TIMETABLE")).unwrap();
        let rows = Timetables::from_timetable(&source).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_TIMETABLE")).unwrap();
        let word = |at| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"TTBL01\0\0");
        assert_eq!(
            (word(8), word(12), word(16), word(20), word(24)),
            (1, 624, 2400, 24, 0x027f)
        );
        let base = 28 + 624 * 16;
        assert_eq!(bytes.len(), base + 2400 * 4);
        for objective in 0..100 {
            for hour in 0..24 {
                assert_eq!(
                    rows.action(objective, hour),
                    Some(word(base + (objective * 24 + hour) * 4)),
                    "objective {objective}, hour {hour}"
                );
            }
        }
        assert_eq!(rows.action(100, 0), None);
        assert_eq!(rows.action(0, 24), None);
    }

    #[test]
    #[ignore = "requires original-instruction resident dispatch fixture"]
    fn matches_native_resident_dispatch() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_DISPATCH")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTRDSP01");
        assert_eq!((word(8), word(12)), (600, 0x14a));
        let demand = std::array::from_fn(|i| word(16 + i * 4));
        let mut at = 36;
        for case in 0..600 {
            let action = word(at) as i32;
            at += 4;
            let mut resident = Resident::from_bytes(bytes[at..at + 0x14a].try_into().unwrap());
            at += 0x14a;
            let mut tables = Timetables {
                rows: [[-1; 24]; 100],
            };
            tables.rows[7][23] = action;
            resident.refresh_intent(&tables, 23, demand).unwrap();
            assert_eq!(
                resident.as_bytes().as_slice(),
                &bytes[at..at + 0x14a],
                "case {case}, action {action}"
            );
            at += 0x14a;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original commodity settings and native membership fixture"]
    fn matches_native_commodity_memberships() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let source = std::fs::read_to_string(crate::fixtures::path("COMMODITY_SETTINGS")).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_MEMBERSHIP")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTCMEM01");
        assert_eq!((word(8), word(12), bytes.len()), (2, 32, 96));
        for at in [16, 56] {
            let initial = std::array::from_fn(|i| word(at + i * 4));
            let expected = std::array::from_fn(|i| word(at + 20 + i * 4));
            assert_eq!(
                apply_commodity_memberships(&source, &image, initial).unwrap(),
                expected
            );
        }
    }

    #[test]
    #[ignore = "requires original-instruction housing category fixture"]
    fn matches_native_housing_score() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_HOUSING_SCORE")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(&bytes[..8], b"MTHSCO01");
        assert_eq!((word(8), word(12)), (139, 0x037f));
        let mut at = 16;
        for case in 0..word(8) {
            let current = usize::try_from(word(at) as i32).ok();
            let dirty = word(at + 8) != 0;
            let inputs = HousingScoreInputs {
                affordability_factor: float(at + 12),
                welfare: float(at + 16),
                rent_weight: float(at + 20),
                district_weight: float(at + 24),
                district_value: float(at + 28),
                average_rent: float(at + 32),
                current_category: word(at + 4) != 0,
            };
            let count = word(at + 36) as usize;
            let expected = word(at + 40) as i32;
            let remains_dirty = word(at + 44) != 0;
            let expected_average = word(at + 48);
            at += 52;
            let mut candidates = Vec::new();
            let mut active = Vec::new();
            for _ in 0..count {
                candidates.push(HousingCandidate {
                    residential: word(at) == 1,
                    capacity: word(at + 4) as u16,
                    occupied: (word(at + 4) >> 16) as u16,
                    attribute_2c: 0,
                    rent: float(at + 12),
                });
                active.push(word(at + 8) != 0);
                at += 16;
            }
            let mut businesses = vec![[0_u8; 544]; 600];
            let mut buildings = vec![[0_u8; 64]; 600];
            for record in &mut businesses {
                record[8..10].fill(255);
            }
            for (i, candidate) in candidates.iter().enumerate() {
                let record = &mut businesses[i];
                record[..4].copy_from_slice(&u32::from(candidate.residential).to_le_bytes());
                record[0xe..0x10].copy_from_slice(&candidate.capacity.to_le_bytes());
                record[0x1dc..0x1e0].copy_from_slice(&candidate.rent.to_le_bytes());
                buildings[i][0x1c..0x20].copy_from_slice(&(i as u32).to_le_bytes());
                buildings[i][0x3f] = u8::from(active[i]);
            }
            let catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| vec![]),
                fractions: std::array::from_fn(|_| vec![]),
            };
            let commodities = [Commodity::default(); 32];
            let market_inputs = crate::market::Inputs {
                businesses: &businesses,
                buildings: &buildings,
                catalogs: &catalogs,
                commodities: &commodities,
            };
            let mut cache = crate::market::Cache {
                dirty,
                prices: [0.; 32],
                rent: inputs.average_rent,
            };
            assert_eq!(
                score_housing_category_cached(&candidates, current, inputs, || cache
                    .residential_rent(&market_inputs))
                .unwrap(),
                expected,
                "score case {case}"
            );
            assert_eq!(cache.dirty, remains_dirty, "dirty case {case}");
            assert_eq!(
                cache.rent.to_bits(),
                expected_average,
                "average case {case}"
            );
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original-instruction housing selection fixture"]
    fn matches_native_housing_selection() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_HOUSING_SELECTION")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(&bytes[..8], b"MTHSEL01");
        assert_eq!((word(8), word(12)), (21, 0x037f));
        let mut at = 16;
        for case in 0..21 {
            let params = [float(at), float(at + 4), float(at + 8), float(at + 12)];
            let current = usize::try_from(word(at + 16) as i32).ok();
            let count = word(at + 20) as usize;
            let expected = usize::try_from(word(at + 24) as i32).ok();
            at += 28;
            let mut candidates = Vec::new();
            for _ in 0..count {
                candidates.push(HousingCandidate {
                    residential: word(at) == 1,
                    capacity: word(at + 4) as u16,
                    occupied: (word(at + 4) >> 16) as u16,
                    attribute_2c: word(at + 8) as i8,
                    rent: float(at + 12),
                });
                at += 16;
            }
            assert_eq!(
                select_residence(
                    &candidates,
                    current,
                    params[0],
                    params[1],
                    params[2],
                    params[3]
                )
                .unwrap(),
                expected,
                "case {case}"
            );
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires both original saves and hour-5 financial observations"]
    fn matches_native_resident_daily_finances() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_FINANCES")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTNH5PFX");
        assert_eq!(
            (word(8), word(12), word(16), word(20), word(24)),
            (2, 2, 2000, 0x90, 0x14a)
        );
        let source = std::fs::read_to_string(crate::fixtures::path("DEFAULT_SETTINGS")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let preferences = Preferences::from_v14_defaults(&source, &image).unwrap();
        let welfare = Welfare::from_v14_defaults(&source).unwrap();
        let mut at = 32;
        for (case, variable) in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"]
            .iter()
            .enumerate()
        {
            let saved_bytes = std::fs::read(crate::fixtures::path(variable)).unwrap();
            let save = crate::save::SaveFile::parse(&saved_bytes).unwrap();
            let saved = save.residents().unwrap();
            let mut residents = crate::economy::ResidentAccounts::from_saved(&saved);
            assert_eq!(word(at + 32) as usize, residents.nodes.len());
            assert_eq!((word(at + 36), word(at + 40)), (0x150, word(at + 32)));
            assert_eq!((word(at + 44), word(at + 48)), (2000 * 0x90, 2000 * 0x90));
            at += 56;
            assert_eq!(saved.accounts, &bytes[at..at + 2000 * 0x90]);
            let expected = &bytes[at + 2000 * 0x90..at + 4000 * 0x90];
            at += 4000 * 0x90;
            for (i, node) in residents.nodes.iter().enumerate() {
                assert_eq!(word(at) as usize, i);
                assert_eq!(word(at + 8), node.account_index() as u32);
                assert_eq!(word(at + 12), node.age());
                assert_eq!(word(at + 16), node.class());
                assert_eq!(word(at + 20) as i32, i32::from(node.preference_index()));
                assert_eq!(word(at + 24) as i32, node.next_in_housing());
                assert_eq!(word(at + 28), 0x14a);
                at += 36;
                assert_eq!(node.as_bytes().as_slice(), &bytes[at..at + 0x14a]);
                at += 0x14a;
            }
            residents
                .update_daily_finances(
                    &welfare,
                    &preferences,
                    &save.economy().unwrap().resident_commodities(),
                )
                .unwrap();
            for (i, (account, native)) in residents
                .accounts
                .iter()
                .zip(expected.chunks_exact(0x90))
                .enumerate()
            {
                assert_eq!(
                    account.as_bytes().as_slice(),
                    native,
                    "save{case} account{i}"
                );
            }
            for (node, original) in residents.nodes.iter().zip(&saved.nodes) {
                assert_eq!(
                    node.as_bytes(),
                    *original,
                    "financial prefix preserves resident bytes"
                );
            }
        }
        assert_eq!(word(28), 0x027f);
        assert_eq!(&bytes[at..at + 8], b"MTWELF01");
        at += 8;
        for amount in welfare.amounts.iter().flatten() {
            assert_eq!(amount.to_bits(), word(at), "native Welfare binding");
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original resident redistribution observations"]
    fn matches_native_resident_redistribution() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_REDISTRIBUTION")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let source = std::fs::read_to_string(crate::fixtures::path("DEFAULT_SETTINGS")).unwrap();
        let defaults = Preferences::from_v14_defaults(&source, &image).unwrap();
        let save_bytes = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&save_bytes).unwrap();
        let commodities = save.economy().unwrap().resident_commodities();
        assert_eq!(&bytes[..8], b"MTNRD2\0\0");
        assert_eq!((word(8), word(16), word(20), word(24)), (1, 0x90, 20, 32));
        let mut at = 32;
        let mut coverage = 0;
        for case in 0..word(12) {
            let preferences = defaults.get((word(at) as i32).try_into().unwrap()).unwrap();
            let count = word(at + 8) as usize;
            assert_eq!(count, preferences.len());
            assert_eq!(word(at + 16) as usize, count);
            coverage |= word(at + 12);
            at += 24;
            // First row is the independently generated native allocation;
            // the next is the explicit balance-discrepancy input.
            let mut account =
                SpendingAccount::from_bytes(bytes[at + 0x90..at + 0x120].try_into().unwrap());
            let expected = &bytes[at + 0x120..at + 0x1b0];
            at += 0x1b0;
            for preference in preferences {
                assert_eq!(preference.commodity, word(at));
                assert_eq!(preference.factor.to_bits(), word(at + 4));
                let c = commodities[preference.commodity as usize];
                assert_eq!(c.active, bytes[at + 8] != 0);
                assert_eq!(c.value.to_bits(), float(at + 12).to_bits());
                assert_eq!(c.markup.to_bits(), float(at + 16).to_bits());
                at += 20;
            }
            account.redistribute(preferences, &commodities).unwrap();
            assert_eq!(
                account.as_bytes().as_slice(),
                expected,
                "case{case} full redistributed account"
            );
        }
        assert_eq!(
            coverage & 0xff,
            0xff,
            "native forward/reverse/gap/quota/zero coverage"
        );
        assert_eq!(at, bytes.len());
    }

    #[test]
    fn resident_seed_retains_pc64_total_through_cell_stores() {
        let large = 2_f32.powi(60);
        let mut commodities = [Commodity::default(); 32];
        commodities[0] = Commodity {
            active: true,
            value: large,
            markup: 1.,
        };
        commodities[1] = Commodity {
            active: false,
            value: 1.,
            markup: 1.,
        };
        let preferences = [
            Preference {
                commodity: 0,
                factor: 1.,
            },
            Preference {
                commodity: 1,
                factor: 1.,
            },
            Preference {
                commodity: 0,
                factor: -1.,
            },
        ];
        // All three draws have remainder 2. The small inactive contribution
        // survives in the PC64 total between cancelling large contributions,
        // although binary64 loses it. Active duplicate cells still store f32.
        let fraction = 2. * f64::from(f32::from_bits(0x3c23d70a));
        let binary64_total: f64 = [f64::from(large), 1., -f64::from(large)]
            .into_iter()
            .map(|product| fraction * product)
            .sum();
        assert_eq!(binary64_total, 0.);
        let mut account = SpendingAccount::from_bytes([0; 0x90]);
        account.set_float(8, 3.);
        let mut state = 0x3f8f;
        account
            .allocate(&preferences, &commodities, &mut state)
            .unwrap();
        assert_eq!(state, 0x10943b40);
        assert_eq!(account.float(4), 0., "duplicate commodity accumulates");
        assert_eq!(account.float(8), 3., "inactive cell stays untouched");
        assert_eq!(&account.as_bytes()[0x84..0x88], &[0; 4]);
        assert_eq!(account.balance(), 5. / 256.);
        assert_eq!(account.float(0x8c), 5. / 256.);
    }

    #[test]
    #[cfg(feature = "lua4")]
    #[ignore = "requires original default preferences, commodity settings, and parameters"]
    fn resident_seed_matches_pc64_default_preference_74() {
        use crate::economy::{
            ResidentAccounts, ResidentCreationInputs, ResidentCreationState, ResidentModelBinding,
        };
        let read = |name| std::fs::read_to_string(crate::fixtures::path(name)).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let configuration = super::Configuration::from_sources(
            &read("DEFAULT_SETTINGS"),
            &read("PARAMETERS"),
            &read("TIMETABLE"),
            &image,
        )
        .unwrap();
        let mut commodity_state = crate::commodity_script::State::from_v14(&image, [0; 5]).unwrap();
        let mut runtime = crate::lua4::Runtime::new().unwrap();
        let bindings =
            crate::commodity_script::Bindings::register(&mut runtime, &image, 100).unwrap();
        runtime
            .execute(
                "commoditysettings.lua",
                read("COMMODITY_SETTINGS").as_bytes(),
                &mut |id, args| bindings.handle(id, args, &mut commodity_state),
            )
            .unwrap();
        let commodities = commodity_state.resident_inputs();
        let mut account = SpendingAccount::from_bytes([0; 0x90]);
        let mut state = 0x2732a6c9;
        account
            .allocate(
                configuration.preferences.get(74).unwrap(),
                &commodities,
                &mut state,
            )
            .unwrap();
        assert_eq!(account.balance().to_bits(), 0x4202e147);
        assert_eq!(account.float(0x8c).to_bits(), 0x4202e147);
        assert_eq!(state, 0x54092fac);

        // Fresh creation consumes five runtime/placement draws and three
        // personal-field draws before entering the same budget routine.
        let mut state = ResidentCreationState {
            auxiliary: super::AuxiliaryPool::new(1),
            initialization_rng: super::InitializationRng {
                constructor: 0xe8295261,
                movement: 0,
                objective: 0,
            },
            identity_rng: 0,
            preference_rng: 0,
        };
        let personal = configuration.inputs(&commodities, commodity_state.memberships, 0);
        let models = std::array::from_fn(|_| ResidentModelBinding {
            reference: 1,
            definition: [0; 0x74],
        });
        let inputs = ResidentCreationInputs {
            personal: &personal,
            preference_groups: &configuration.preference_groups,
            models: &models,
            countdown_period: 1,
        };
        let mut categories = [[0u8; 0x17c]; 52];
        let mut residents = ResidentAccounts::new_pool();
        let index = residents
            .create_resident([2, 0, 74], [0; 0x11c], &inputs, &mut state, &mut categories)
            .unwrap()
            .unwrap();
        let account_index = usize::try_from(residents.nodes[index].account_index()).unwrap();
        let created = &residents.accounts[account_index];
        assert_eq!(created.balance().to_bits(), 0x4202e147);
        assert_eq!(created.float(0x8c).to_bits(), 0x4202e147);
        assert_eq!(state.initialization_rng.constructor, 0x54092fac);
        assert_eq!(&created.as_bytes()[4..], &account.as_bytes()[4..]);
    }

    #[test]
    #[ignore = "requires original resident allocation observations"]
    fn matches_native_resident_allocations() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_RESIDENT_ALLOCATIONS")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let short = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let source = std::fs::read_to_string(crate::fixtures::path("DEFAULT_SETTINGS")).unwrap();
        let defaults = Preferences::from_v14_defaults(&source, &image).unwrap();
        let save_bytes = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&save_bytes).unwrap();
        let commodities = save.economy().unwrap().resident_commodities();
        assert_eq!(&bytes[..8], b"MTNROWF2");
        assert_eq!((short(8), short(10)), (2, 32));
        assert_eq!((word(16), word(20), word(24)), (0x90, 20, 0x027f));
        let mut at = 32;
        for case in 0..word(12) {
            let preference_index = word(at);
            let count = usize::from(short(at + 4));
            let mut state = word(at + 8);
            assert_eq!((word(at + 12), word(at + 16)), (0x343fd, 0x269ec3));
            let expected_state = word(at + 20);
            assert_eq!((word(at + 24), word(at + 28)), (0x343fd, 0x269ec3));
            at += 32;
            let mut preferences = Vec::new();
            for _ in 0..count {
                let id = word(at) as usize;
                preferences.push(Preference {
                    commodity: id.try_into().unwrap(),
                    factor: float(at + 4),
                });
                assert_eq!(commodities[id].active, bytes[at + 8] != 0);
                assert_eq!(commodities[id].value.to_bits(), word(at + 12));
                assert_eq!(commodities[id].markup.to_bits(), word(at + 16));
                at += 20;
            }
            if preference_index != u32::MAX {
                let actual = defaults.get(preference_index.try_into().unwrap()).unwrap();
                assert_eq!(
                    actual.len(),
                    preferences.len(),
                    "case{case} preference count"
                );
                for (a, b) in actual.iter().zip(&preferences) {
                    assert_eq!(a.commodity, b.commodity);
                    assert_eq!(a.factor.to_bits(), b.factor.to_bits());
                }
            }
            let mut account = SpendingAccount::from_bytes(bytes[at..at + 0x90].try_into().unwrap());
            let preferences = if preference_index == u32::MAX {
                &[][..]
            } else {
                defaults.get(preference_index.try_into().unwrap()).unwrap()
            };
            account
                .allocate(preferences, &commodities, &mut state)
                .unwrap();
            assert_eq!(state, expected_state, "case{case} RNG");
            assert_eq!(
                account.as_bytes().as_slice(),
                &bytes[at + 0x90..at + 0x120],
                "case{case} complete account"
            );
            at += 0x120;
        }
        assert_eq!(&bytes[at..at + 8], b"MTPREF01");
        assert_eq!(word(at + 8), 128);
        at += 12;
        for index in 0..128 {
            let actual = defaults.get(index as i8).unwrap();
            assert_eq!(
                actual.len(),
                word(at + 0x100) as usize,
                "preference list{index}"
            );
            for (slot, preference) in actual.iter().enumerate() {
                assert_eq!(preference.commodity, word(at + slot * 4));
                assert_eq!(preference.factor.to_bits(), word(at + 0x80 + slot * 4));
            }
            at += 0x104;
        }
        assert_eq!(at, bytes.len());
    }
}
