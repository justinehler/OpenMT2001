//! Live building records and original block occupancy, before model binding.
pub const RECORD_SIZE: usize = 0x394;
pub const COUNT: usize = 600;

/// Original445350 uses live BusinessInfo+34 and adds image5251b8 (0.999)
/// before FTOL. This differs from the later stock-capacity calculation.
pub fn capacity(record: &[u8; RECORD_SIZE], rate: f32) -> i32 {
    let volume = (i32::from(record[7]) - i32::from(record[5]) + 1)
        .wrapping_mul(i32::from(record[6]) - i32::from(record[4]) + 1)
        .wrapping_mul(i32::from(record[8]));
    crate::economy::ftol_low_word(f64::from(volume) * f64::from(rate) + 0.999)
}

/// Original4456b0 resets both the temporary record and all600 indexed slots.
pub fn reset(
    records: &mut [[u8; RECORD_SIZE]],
    temporary: &mut [u8; RECORD_SIZE],
) -> Result<(), String> {
    if records.len() != COUNT {
        return Err("building reset requires600 slots".into());
    }
    temporary.fill(0);
    temporary[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    for (slot, record) in records.iter_mut().enumerate() {
        record.fill(0);
        record[..4].copy_from_slice(&(slot as u32).to_le_bytes());
    }
    Ok(())
}

/// Original445650 finds storage but does not mark it active. The constructor
/// performs that write; callers must not treat this lookup as a reservation.
pub fn first_free(records: &[[u8; RECORD_SIZE]]) -> Result<Option<usize>, String> {
    if records.len() != COUNT {
        return Err("building allocation requires600 slots".into());
    }
    Ok(records.iter().position(|record| record[0x3f] == 0))
}

/// Original445f50 links an already initialized building. References stored in
/// records are logical arena addresses, with `base` identifying slot zero.
/// This helper does not perform445fb0's geometry/occupancy/category assignment.
pub fn append_category(
    category: &mut [u8; 0x17c],
    records: &mut [[u8; RECORD_SIZE]],
    slot: usize,
    base: u32,
) -> Result<(), String> {
    if records.len() != COUNT || slot >= COUNT || base == 0 {
        return Err("invalid building arena for category append".into());
    }
    let reference = base
        .checked_add((slot * RECORD_SIZE) as u32)
        .ok_or("building reference overflow")?;
    let head = u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap());
    let tail = u32::from_le_bytes(category[0x160..0x164].try_into().unwrap());
    if head == 0 {
        category[0x15c..0x160].copy_from_slice(&reference.to_le_bytes());
        category[0x160..0x164].copy_from_slice(&reference.to_le_bytes());
        records[slot][0x48..0x4c].fill(0);
    } else {
        let offset = tail
            .checked_sub(base)
            .ok_or("category tail precedes building arena")? as usize;
        if !offset.is_multiple_of(RECORD_SIZE) || offset / RECORD_SIZE >= COUNT {
            return Err("category tail leaves building arena".into());
        }
        records[offset / RECORD_SIZE][0x44..0x48].copy_from_slice(&reference.to_le_bytes());
        records[slot][0x48..0x4c].copy_from_slice(&tail.to_le_bytes());
        category[0x160..0x164].copy_from_slice(&reference.to_le_bytes());
    }
    let count = u32::from_le_bytes(category[8..12].try_into().unwrap());
    records[slot][0x18] = count as u8;
    category[8..12].copy_from_slice(&count.wrapping_add(1).to_le_bytes());
    records[slot][0x44..0x48].fill(0);
    Ok(())
}

pub trait GeometryBinding {
    /// Original444ff0 prepares geometry after the category reference is set.
    fn prepare(&mut self, slot: usize, record: &mut [u8; RECORD_SIZE]) -> Result<(), String>;
    /// Resolve the original model handle stored at building+170.
    fn model_height(&mut self, handle: u32) -> Result<f32, String>;
}

/// Original447a90's category bounds write. The same bounds refresh runs after
/// both insertion and removal, so it is kept as one numeric state helper.
fn refresh_category_bounds(category: &mut [u8; 0x17c], height: f32) {
    for corner in 0..8 {
        let point = [
            if corner & 1 == 0 { -41_f32 } else { 41. },
            if corner & 2 == 0 { 0. } else { height },
            if corner & 4 == 0 { -41. } else { 41. },
        ];
        for (axis, value) in point.into_iter().enumerate() {
            let at = 0xe8 + corner * 12 + axis * 4;
            category[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    let half = f64::from(height) * 0.5;
    let radius_squared = half * half + 3362.;
    for (offset, value) in [
        (0x148, 0_f32),
        (0x14c, half as f32),
        (0x150, 0.),
        (0x154, radius_squared as f32),
        (0x158, radius_squared.sqrt() as f32),
    ] {
        category[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// Original445fb0 around its geometry dependency. Occupancy, category bounds,
/// and entrance translation are live simulation state, not renderer guesses.
pub fn bind_category(
    category: &mut [u8; 0x17c],
    records: &mut [[u8; RECORD_SIZE]],
    slot: usize,
    building_base: u32,
    category_reference: u32,
    geometry: &mut impl GeometryBinding,
) -> Result<(), String> {
    if records.len() != COUNT || slot >= COUNT || category_reference == 0 {
        return Err("invalid category/building storage".into());
    }
    records[slot][0x40..0x44].copy_from_slice(&category_reference.to_le_bytes());
    geometry.prepare(slot, &mut records[slot])?;
    append_category(category, records, slot, building_base)?;
    let rectangle = std::array::from_fn(|i| i32::from(records[slot][4 + i]));
    fill_occupancy(
        (&mut category[0x9c..0xcd]).try_into().unwrap(),
        rectangle,
        records[slot][8],
    )?;
    let mut reference = u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap());
    let mut height = 10_f32;
    let mut visited = 0;
    while reference != 0 {
        let offset = reference
            .checked_sub(building_base)
            .ok_or("building link precedes arena")? as usize;
        if !offset.is_multiple_of(RECORD_SIZE) || offset / RECORD_SIZE >= COUNT || visited >= COUNT
        {
            return Err("invalid or cyclic category building chain".into());
        }
        let record = &records[offset / RECORD_SIZE];
        let candidate =
            geometry.model_height(u32::from_le_bytes(record[0x170..0x174].try_into().unwrap()))?;
        if candidate > height {
            height = candidate;
        }
        reference = u32::from_le_bytes(record[0x44..0x48].try_into().unwrap());
        visited += 1;
    }
    refresh_category_bounds(category, height);
    let record = &mut records[slot];
    let count = i32::from_le_bytes(record[0x220..0x224].try_into().unwrap());
    if count > ((RECORD_SIZE - 0x224) / 12) as i32 {
        return Err("building entrances exceed runtime storage".into());
    }
    let delta: [f32; 2] = std::array::from_fn(|axis| {
        let a = 0xe0 + axis * 4;
        let b = 0xc + axis * 4;
        (f64::from(f32::from_le_bytes(category[a..a + 4].try_into().unwrap()))
            - f64::from(f32::from_le_bytes(record[b..b + 4].try_into().unwrap()))) as f32
    });
    for entrance in 0..count.max(0) as usize {
        for (axis, shift) in delta.into_iter().enumerate() {
            let at = 0x224 + entrance * 12 + axis * 8;
            let value = f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
            record[at..at + 4]
                .copy_from_slice(&((f64::from(value) - f64::from(shift)) as f32).to_le_bytes());
        }
    }
    Ok(())
}

/// Host boundaries surrounding original resource/model release and refresh
/// calls. Resource/model callbacks receive `(slot, native reference, record)`;
/// this keeps the selected mutable record available for retained renderer
/// state cleanup while preserving the original pointer/index argument.
pub type RemovalRecordCallback<'a> =
    &'a mut dyn FnMut(usize, u32, &mut [u8; RECORD_SIZE]) -> Result<(), String>;

pub struct RemovalCallbacks<'a> {
    pub release_resource: RemovalRecordCallback<'a>,
    pub release_model: RemovalRecordCallback<'a>,
    pub refresh: &'a mut dyn FnMut() -> Result<(), String>,
}

/// Mutable global pointers used by original445d10 when the selected building
/// is also the process-wide current building. They are references rather than
/// hidden module state so callers can bind them to the live world owner.
pub struct RemovalGlobals<'a> {
    pub global_590f60: &'a mut u32,
    pub global_6175f8: &'a mut u32,
}

/// The fixed-size auxiliary record consumed by original0047e450.  The
/// renderer and auxiliary registries live outside this record; callers bind
/// those stores through `AuxiliaryCleanupCallbacks` below.
pub const AUXILIARY_RECORD_SIZE: usize = 0x340;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxiliaryArray {
    First,
    Second,
    Third,
}

/// Host boundaries for the external stores touched by original0047e450.
/// Values passed as `index` are the actual registry indices stored in the
/// auxiliary record.  The array callback also receives the original array
/// and element position, preserving the native release order.
pub struct AuxiliaryCleanupCallbacks<'a> {
    pub release_array: &'a mut dyn FnMut(AuxiliaryArray, usize, u32) -> Result<(), String>,
    pub release_renderer: &'a mut dyn FnMut(usize) -> Result<(), String>,
    pub release_aux_registry: &'a mut dyn FnMut(usize) -> Result<(), String>,
    pub refresh: &'a mut dyn FnMut(&mut [u8; AUXILIARY_RECORD_SIZE]) -> Result<(), String>,
}

fn auxiliary_word(record: &[u8; AUXILIARY_RECORD_SIZE], offset: usize) -> u32 {
    u32::from_le_bytes(record[offset..offset + 4].try_into().unwrap())
}

fn set_auxiliary_word(record: &mut [u8; AUXILIARY_RECORD_SIZE], offset: usize, value: u32) {
    record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Execute the record-owned portion of original0047e450.
///
/// The native body releases three inline arrays, calls original00485880,
/// releases renderer indices, then auxiliary-registry indices, and finally
/// clears the two count words.  External tables remain callback-owned so the
/// live renderer can clear its own rows synchronously.
pub fn cleanup_auxiliary_record(
    record: &mut [u8; AUXILIARY_RECORD_SIZE],
    callbacks: &mut AuxiliaryCleanupCallbacks<'_>,
) -> Result<bool, String> {
    if auxiliary_word(record, 0) != 1 {
        return Ok(false);
    }

    for (array, values, count_offset) in [
        (AuxiliaryArray::First, 0x124, 0x130),
        (AuxiliaryArray::Second, 0x134, 0x140),
        (AuxiliaryArray::Third, 0x144, 0x150),
    ] {
        let count = auxiliary_word(record, count_offset) as usize;
        if count > (AUXILIARY_RECORD_SIZE - values) / 4 {
            return Err(format!("auxiliary array count exceeds record: {count}"));
        }
        for index in 0..count {
            (callbacks.release_array)(array, index, auxiliary_word(record, values + index * 4))?;
        }
    }

    (callbacks.refresh)(record)?;

    let renderer_count = auxiliary_word(record, 0x298) as usize;
    if renderer_count > (AUXILIARY_RECORD_SIZE - 0x29c) / 4 {
        return Err(format!(
            "renderer index count exceeds record: {renderer_count}"
        ));
    }
    for index in 0..renderer_count {
        (callbacks.release_renderer)(auxiliary_word(record, 0x29c + index * 4) as usize)?;
    }
    // Native clears +0x298 before it begins the auxiliary-registry loop.
    set_auxiliary_word(record, 0x298, 0);

    let registry_count = auxiliary_word(record, 0x31c) as usize;
    if registry_count > (AUXILIARY_RECORD_SIZE - 0x320) / 4 {
        return Err(format!(
            "auxiliary registry count exceeds record: {registry_count}"
        ));
    }
    for index in 0..registry_count {
        (callbacks.release_aux_registry)(auxiliary_word(record, 0x320 + index * 4) as usize)?;
    }

    set_auxiliary_word(record, 0x31c, 0);
    Ok(true)
}

/// Original445d10 removes a category building by its identity at record+0.
/// The selected record remains in its pool slot with stale link words, matching
/// the native slot cleanup; only its active byte, model index, and occupancy
/// contribution are cleared.
pub fn remove_category_building(
    category: &mut [u8; 0x17c],
    records: &mut [[u8; RECORD_SIZE]],
    identity: u32,
    building_base: u32,
    globals: &mut RemovalGlobals<'_>,
    geometry: &mut impl GeometryBinding,
    callbacks: &mut RemovalCallbacks<'_>,
) -> Result<Option<usize>, String> {
    if records.len() != COUNT || building_base == 0 {
        return Err("invalid building removal storage".into());
    }
    let mut reference = u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap());
    let mut selected = None;
    let mut visited = 0;
    while reference != 0 {
        let offset = reference
            .checked_sub(building_base)
            .ok_or("category link precedes building arena")? as usize;
        if !offset.is_multiple_of(RECORD_SIZE) || offset / RECORD_SIZE >= COUNT {
            return Err("invalid category building chain".into());
        }
        let slot = offset / RECORD_SIZE;
        if u32::from_le_bytes(records[slot][..4].try_into().unwrap()) == identity {
            selected = Some((slot, reference));
            break;
        }
        reference = u32::from_le_bytes(records[slot][0x44..0x48].try_into().unwrap());
        visited += 1;
        if visited >= COUNT {
            return Err("cyclic category building chain".into());
        }
    }
    let Some((slot, selected_reference)) = selected else {
        return Ok(None);
    };

    let next = u32::from_le_bytes(records[slot][0x44..0x48].try_into().unwrap());
    let previous = u32::from_le_bytes(records[slot][0x48..0x4c].try_into().unwrap());
    let head = u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap());
    let tail = u32::from_le_bytes(category[0x160..0x164].try_into().unwrap());
    if selected_reference == head {
        if next == 0 {
            category[0x15c..0x160].fill(0);
            category[0x160..0x164].fill(0);
        } else {
            category[0x15c..0x160].copy_from_slice(&next.to_le_bytes());
            let next_slot = slot_from_reference(next, building_base)?;
            records[next_slot][0x48..0x4c].fill(0);
        }
    } else {
        let next_after = if selected_reference == tail {
            category[0x160..0x164].copy_from_slice(&previous.to_le_bytes());
            0
        } else {
            next
        };
        let previous_slot = slot_from_reference(previous, building_base)?;
        records[previous_slot][0x44..0x48].copy_from_slice(&next_after.to_le_bytes());
        if next_after != 0 {
            let next_slot = slot_from_reference(next_after, building_base)?;
            records[next_slot][0x48..0x4c].copy_from_slice(&previous.to_le_bytes());
        }
    }
    if *globals.global_590f60 == selected_reference {
        *globals.global_590f60 = 0;
    }
    if *globals.global_6175f8 == selected_reference {
        *globals.global_6175f8 = 0;
    }
    let count = u32::from_le_bytes(category[8..12].try_into().unwrap());
    category[8..12].copy_from_slice(&count.wrapping_sub(1).to_le_bytes());

    records[slot][0x3f] = 0;
    let [x0, y0, x1, y1] = [
        records[slot][4],
        records[slot][5],
        records[slot][6],
        records[slot][7],
    ];
    if x0 <= x1 && y0 <= y1 {
        if x1 >= 7 || y1 >= 7 {
            return Err("building removal rectangle leaves7x7 block".into());
        }
        let occupancy: &mut [u8; 49] = (&mut category[0x9c..0xcd]).try_into().unwrap();
        for x in x0..=x1 {
            for y in y0..=y1 {
                occupancy[(x as usize) * 7 + y as usize] = 0;
            }
        }
    }
    let resource_reference = building_base
        .checked_add((slot * RECORD_SIZE) as u32)
        .and_then(|reference| reference.checked_add(0x4c))
        .ok_or("building resource reference overflow")?;
    (callbacks.release_resource)(slot, resource_reference, &mut records[slot])?;
    let model_index = u32::from_le_bytes(records[slot][0x7c..0x80].try_into().unwrap());
    if model_index != u32::MAX {
        (callbacks.release_model)(slot, model_index, &mut records[slot])?;
        records[slot][0x7c..0x80].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    let mut reference = u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap());
    let mut height = 10_f32;
    let mut visited = 0;
    while reference != 0 {
        let remaining_slot = slot_from_reference(reference, building_base)?;
        let handle = u32::from_le_bytes(records[remaining_slot][0x170..0x174].try_into().unwrap());
        let candidate = geometry.model_height(handle)?;
        if candidate > height {
            height = candidate;
        }
        reference = u32::from_le_bytes(records[remaining_slot][0x44..0x48].try_into().unwrap());
        visited += 1;
        if visited >= COUNT {
            return Err("cyclic category building chain".into());
        }
    }
    refresh_category_bounds(category, height);
    (callbacks.refresh)()?;
    Ok(Some(slot))
}

fn slot_from_reference(reference: u32, building_base: u32) -> Result<usize, String> {
    let offset = reference
        .checked_sub(building_base)
        .ok_or("building link precedes arena")? as usize;
    if !offset.is_multiple_of(RECORD_SIZE) || offset / RECORD_SIZE >= COUNT {
        return Err("building link leaves arena".into());
    }
    Ok(offset / RECORD_SIZE)
}

/// Original445b10 validates inclusive endpoints and requires an outside edge.
pub fn can_place(occupancy: &[u8; 49], [x0, y0, x1, y1]: [i32; 4]) -> bool {
    if x0 > x1 || y0 > y1 || x0 < 0 || y0 < 0 || x1 >= 7 || y1 >= 7 {
        return false;
    }
    if x0 != 0 && y0 != 0 && x1 != 6 && y1 != 6 {
        return false;
    }
    (x0..=x1).all(|x| (y0..=y1).all(|y| occupancy[(x * 7 + y) as usize] == 0))
}

/// Original445b90 fills x-major occupancy with height;445be0 uses zero.
/// Callers validate placement separately. Clearing an occupied interior range
/// is valid here, so this does not invoke the placement predicate.
pub fn fill_occupancy(
    occupancy: &mut [u8; 49],
    [x0, y0, x1, y1]: [i32; 4],
    height: u8,
) -> Result<(), String> {
    if x0 > x1 || y0 > y1 {
        return Ok(());
    }
    if x0 < 0 || y0 < 0 || x1 >= 7 || y1 >= 7 {
        return Err("occupancy rectangle leaves7x7 block".into());
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            occupancy[(x * 7 + y) as usize] = height;
        }
    }
    Ok(())
}

pub struct Initialization {
    pub rectangle: [i32; 4],
    pub height: u32,
    pub center: [f32; 2],
    pub variation: u32,
    pub kind: u32,
    pub quality: i8,
    /// Raw f32 argument bits copied by4450e0 to+38. The Lua AddBuilding
    /// caller converts its FTOL integer construction cost back to f32 first.
    pub total_cost: u32,
    /// Low bytes of822958 and822954, in that order.
    pub date: [u8; 2],
}

/// Original446130 center calculation, including the X-origin f32 spill.
pub fn center(
    category: &[u8; 0x17c],
    rectangle: [i32; 4],
    image: &crate::pe::PeImage<'_>,
) -> Result<[f32; 2], String> {
    let shift = f64::from(image.f32_at(0x525098)?);
    let ten = f64::from(image.f32_at(0x5250a0)?);
    let half = f64::from(image.f32_at(0x52502c)?);
    Ok(std::array::from_fn(|axis| {
        let at = 0xe0 + axis * 4;
        let origin =
            f64::from(f32::from_le_bytes(category[at..at + 4].try_into().unwrap())) - shift;
        let origin = if axis == 0 {
            f64::from(origin as f32)
        } else {
            origin
        };
        ((f64::from(rectangle[axis + 2].wrapping_add(1)) * ten
            + origin
            + (f64::from(rectangle[axis]) * ten + origin))
            * half) as f32
    }))
}

/// Original4450e0 preserves the slot identity and unrelated runtime bytes.
/// Category links/model geometry are the subsequent445fb0 stage.
pub fn initialize(
    record: &mut [u8; RECORD_SIZE],
    input: &Initialization,
    image: &crate::pe::PeImage<'_>,
) -> Result<(), String> {
    let address = input
        .kind
        .checked_mul(0x2e)
        .and_then(|n| n.checked_add(0x5365f0))
        .ok_or("building type address overflow")?;
    let unit_price = u32::from(u16::from_le_bytes(
        image.at(address, 2)?.try_into().unwrap(),
    ));
    initialize_rent_word(record, input, unit_price);
    Ok(())
}
/// New buildings copy configured rent; restored records never call this.
pub fn initialize_with_parameters(
    record: &mut [u8; RECORD_SIZE],
    input: &Initialization,
    parameters: &crate::building_parameters::BuildingKinds,
) -> Result<(), String> {
    initialize_rent_word(record, input, u32::from(parameters.row(input.kind)?[0]));
    Ok(())
}
fn initialize_rent_word(record: &mut [u8; RECORD_SIZE], input: &Initialization, unit_price: u32) {
    for (offset, value) in (4..8).zip(input.rectangle) {
        record[offset] = value as u8;
    }
    record[8] = input.height as u8;
    for (offset, value) in [
        (0xc, input.center[0].to_bits()),
        (0x10, input.center[1].to_bits()),
        (0x20, input.variation),
        (0x24, input.kind),
        (0x14, 6),
        (0x1c, u32::MAX),
        (0x28, unit_price),
        (0x38, input.total_cost),
    ] {
        record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    record[0x2c] = input.quality as u8;
    record[0x3f] = 1;
    record[0x2d] = 0;
    record[0x34] = input.date[0].wrapping_mul(10).wrapping_add(input.date[1]);
    record[0x3c..0x3f].fill(0);
}

/// Resolve current live category links for read-only consumers such as the
/// initial AI count pass. The result is a call-boundary view, not retained
/// topology; subsequent building mutations must resolve the links again.
pub fn category_chains(
    categories: &[[u8; 0x17c]; 52],
    buildings: &[[u8; RECORD_SIZE]],
    base: u32,
) -> Result<[Vec<usize>; 52], String> {
    let mut result = std::array::from_fn(|_| Vec::new());
    for (category, chain) in categories.iter().zip(result.iter_mut()) {
        let mut reference = u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap());
        while reference != 0 {
            let offset = reference
                .checked_sub(base)
                .ok_or("category link precedes building arena")? as usize;
            if !offset.is_multiple_of(RECORD_SIZE) {
                return Err("unaligned category building link".into());
            }
            let index = offset / RECORD_SIZE;
            let building = buildings
                .get(index)
                .ok_or("category link leaves building arena")?;
            if chain.contains(&index) {
                return Err("cyclic category building links".into());
            }
            chain.push(index);
            reference = u32::from_le_bytes(building[0x44..0x48].try_into().unwrap());
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[test]
    #[ignore = "requires original445fb0 wrapper and447a90 bounds observations"]
    fn matches_native_category_binding() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_CATEGORY_BINDING")).unwrap();
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        struct Geometry {
            center: [f32; 2],
            footprint: [u8; 5],
            handle: u32,
            heights: Vec<(u32, f32)>,
            positions: Vec<[f32; 3]>,
            calls: usize,
        }
        impl GeometryBinding for Geometry {
            fn prepare(
                &mut self,
                slot: usize,
                record: &mut [u8; RECORD_SIZE],
            ) -> Result<(), String> {
                assert_eq!(slot, 2);
                assert_eq!(
                    u32::from_le_bytes(record[0x40..0x44].try_into().unwrap()),
                    0x617628
                );
                self.calls += 1;
                record[4..9].copy_from_slice(&self.footprint);
                for (axis, value) in self.center.into_iter().enumerate() {
                    let at = 0xc + axis * 4;
                    record[at..at + 4].copy_from_slice(&value.to_le_bytes());
                }
                record[0x170..0x174].copy_from_slice(&self.handle.to_le_bytes());
                record[0x220..0x224].copy_from_slice(&(self.positions.len() as u32).to_le_bytes());
                for (index, point) in self.positions.iter().enumerate() {
                    for (axis, value) in point.iter().enumerate() {
                        let at = 0x224 + index * 12 + axis * 4;
                        record[at..at + 4].copy_from_slice(&value.to_le_bytes());
                    }
                }
                Ok(())
            }
            fn model_height(&mut self, handle: u32) -> Result<f32, String> {
                self.heights
                    .iter()
                    .find(|entry| entry.0 == handle)
                    .map(|entry| entry.1)
                    .ok_or("unknown model".into())
            }
        }
        assert_eq!(&bytes[..8], b"MTCATB01");
        let mut at = 8;
        assert_eq!(word(&mut at), 1);
        let count = word(&mut at);
        assert_eq!(word(&mut at), 0x17c);
        assert_eq!(word(&mut at), RECORD_SIZE as u32);
        for case in 0..count {
            let name_len = word(&mut at) as usize;
            at += name_len;
            let _nonempty = word(&mut at);
            let _category_center = [word(&mut at), word(&mut at)];
            let center = [f32::from_bits(word(&mut at)), f32::from_bits(word(&mut at))];
            let footprint = bytes[at..at + 5].try_into().unwrap();
            at += 5;
            let handle = word(&mut at);
            let _old_handle = word(&mut at);
            let _new_height = word(&mut at);
            let heights = (0..word(&mut at))
                .map(|_| (word(&mut at), f32::from_bits(word(&mut at))))
                .collect();
            let positions = (0..word(&mut at))
                .map(|_| std::array::from_fn(|_| f32::from_bits(word(&mut at))))
                .collect();
            let mut geometry = Geometry {
                center,
                footprint,
                handle,
                heights,
                positions,
                calls: 0,
            };
            assert_eq!(word(&mut at), 0x17c);
            let mut category: [u8; 0x17c] = bytes[at..at + 0x17c].try_into().unwrap();
            at += 0x17c;
            assert_eq!(word(&mut at), 0x17c);
            let expected_category = &bytes[at..at + 0x17c];
            at += 0x17c;
            let rows = word(&mut at) as usize;
            let mut records = vec![[0; RECORD_SIZE]; COUNT];
            let mut expected = Vec::new();
            for row in records.iter_mut().take(rows) {
                assert_eq!(word(&mut at), RECORD_SIZE as u32);
                row.copy_from_slice(&bytes[at..at + RECORD_SIZE]);
                at += RECORD_SIZE;
                assert_eq!(word(&mut at), RECORD_SIZE as u32);
                expected.push(&bytes[at..at + RECORD_SIZE]);
                at += RECORD_SIZE;
            }
            let events = word(&mut at);
            assert_eq!(events, 2);
            for index in 0..events {
                let len = word(&mut at) as usize;
                assert_eq!(
                    &bytes[at..at + len],
                    if index == 0 { b"00444ff0" } else { b"00447a90" }
                );
                at += len;
                let _event = [word(&mut at), word(&mut at), word(&mut at)];
            }
            bind_category(
                &mut category,
                &mut records,
                2,
                0x591318,
                0x617628,
                &mut geometry,
            )
            .unwrap();
            assert_eq!(geometry.calls, 1);
            for (offset, (actual, expected)) in category.iter().zip(expected_category).enumerate() {
                assert_eq!(actual, expected, "category case {case}, offset {offset:x}");
            }
            for (row, expected) in records.iter().zip(expected) {
                for (offset, (actual, expected)) in row.iter().zip(expected).enumerate() {
                    assert_eq!(actual, expected, "building case {case}, offset {offset:x}");
                }
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original445f50 category/building link observations"]
    fn matches_native_category_append() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_CATEGORY_BUILDING_LINK")).unwrap();
        let word = |at: &mut usize| {
            let result = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            result
        };
        assert_eq!(&bytes[..8], b"MTCLNK01");
        let mut at = 8;
        assert_eq!(word(&mut at), 1);
        let count = word(&mut at);
        assert_eq!(word(&mut at), 0x17c);
        assert_eq!(word(&mut at), RECORD_SIZE as u32);
        for case in 0..count {
            let name_len = word(&mut at) as usize;
            at += name_len;
            let _count = word(&mut at);
            let _tail = word(&mut at);
            let slot = word(&mut at) as usize;
            assert_eq!(word(&mut at), 0x17c);
            let mut category: [u8; 0x17c] = bytes[at..at + 0x17c].try_into().unwrap();
            at += 0x17c;
            assert_eq!(word(&mut at), 0x17c);
            let expected_category = &bytes[at..at + 0x17c];
            at += 0x17c;
            let rows = word(&mut at) as usize;
            let mut records = vec![[0; RECORD_SIZE]; COUNT];
            let mut expected = Vec::new();
            for record in records.iter_mut().take(rows) {
                assert_eq!(word(&mut at), RECORD_SIZE as u32);
                record.copy_from_slice(&bytes[at..at + RECORD_SIZE]);
                at += RECORD_SIZE;
                assert_eq!(word(&mut at), RECORD_SIZE as u32);
                expected.push(&bytes[at..at + RECORD_SIZE]);
                at += RECORD_SIZE;
            }
            append_category(&mut category, &mut records, slot, 0x591318).unwrap();
            assert_eq!(
                category.as_slice(),
                expected_category,
                "category case {case}"
            );
            for (row, expected) in records.iter().zip(expected) {
                assert_eq!(row.as_slice(), expected, "building case {case}");
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original445d10 removal observations"]
    fn matches_native_building_removal() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUILDING_REMOVE")).unwrap();
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        let blob = |at: &mut usize| {
            let len = word(at) as usize;
            let value = bytes[*at..*at + len].to_vec();
            *at += len;
            value
        };
        assert_eq!(&bytes[..8], b"MTBRM02\0");
        let mut at = 8;
        assert_eq!(word(&mut at), 2);
        let cases = word(&mut at);
        for case in 0..cases {
            let _name = blob(&mut at);
            let category_index = word(&mut at);
            let target = word(&mut at) as usize;
            let target_id = word(&mut at);
            let global_target = word(&mut at) != 0;
            let _model_handle = word(&mut at);
            let _resource_index = i32::from_le_bytes(word(&mut at).to_le_bytes());
            let _extents = [
                word(&mut at),
                word(&mut at),
                word(&mut at),
                word(&mut at),
                word(&mut at),
            ];
            let linked_count = word(&mut at) as usize;
            let _linked: Vec<usize> = (0..linked_count).map(|_| word(&mut at) as usize).collect();
            let occupancy_len = word(&mut at) as usize;
            assert_eq!(occupancy_len, 49);
            let _occupancy = bytes[at..at + occupancy_len].to_vec();
            at += occupancy_len;
            let height_count = word(&mut at) as usize;
            let heights: Vec<(u32, f32)> = (0..height_count)
                .map(|_| (word(&mut at), f32::from_bits(word(&mut at))))
                .collect();
            let result = word(&mut at);
            let return_reached = word(&mut at) != 0;
            let fault = word(&mut at) != 0;
            let call_count = word(&mut at) as usize;
            let mut native_calls = Vec::with_capacity(call_count);
            for _ in 0..call_count {
                let address = word(&mut at);
                let ecx = word(&mut at);
                let argc = word(&mut at) as usize;
                let kind = word(&mut at);
                let args: Vec<u32> = (0..argc).map(|_| word(&mut at)).collect();
                native_calls.push((address, ecx, kind, args));
            }
            let mut category: [u8; 0x17c] = blob(&mut at).try_into().unwrap();
            let before_head = word(&mut at);
            let before_tail = word(&mut at);
            let before_count = word(&mut at);
            let mut global_590f60 = word(&mut at);
            let mut global_6175f8 = word(&mut at);
            assert_eq!(
                u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap()),
                before_head
            );
            assert_eq!(
                u32::from_le_bytes(category[0x160..0x164].try_into().unwrap()),
                before_tail
            );
            assert_eq!(
                u32::from_le_bytes(category[8..12].try_into().unwrap()),
                before_count
            );
            let before_globals = [global_590f60, global_6175f8];
            let building_count = word(&mut at) as usize;
            let mut records = vec![[0_u8; RECORD_SIZE]; COUNT];
            let mut selected = Vec::new();
            for _ in 0..building_count {
                let slot = word(&mut at) as usize;
                let len = word(&mut at) as usize;
                assert_eq!(len, RECORD_SIZE);
                records[slot].copy_from_slice(&bytes[at..at + len]);
                at += len;
                selected.push(slot);
            }
            let expected_category = blob(&mut at);
            let expected_head = word(&mut at);
            let expected_tail = word(&mut at);
            let expected_count = word(&mut at);
            let expected_590f60 = word(&mut at);
            let expected_6175f8 = word(&mut at);
            let expected_records = {
                let count = word(&mut at) as usize;
                let mut rows = Vec::new();
                for _ in 0..count {
                    let slot = word(&mut at) as usize;
                    let len = word(&mut at) as usize;
                    assert_eq!(len, RECORD_SIZE);
                    rows.push((slot, bytes[at..at + len].to_vec()));
                    at += len;
                }
                rows
            };
            let write_count = word(&mut at) as usize;
            at += write_count * 12;

            struct Geometry {
                heights: Vec<(u32, f32)>,
            }
            impl GeometryBinding for Geometry {
                fn prepare(
                    &mut self,
                    _slot: usize,
                    _record: &mut [u8; RECORD_SIZE],
                ) -> Result<(), String> {
                    Err("removal must not prepare geometry".into())
                }
                fn model_height(&mut self, handle: u32) -> Result<f32, String> {
                    self.heights
                        .iter()
                        .find(|entry| entry.0 == handle)
                        .map(|entry| entry.1)
                        .ok_or_else(|| format!("unknown model {handle}"))
                }
            }
            let mut geometry = Geometry { heights };
            let events = RefCell::new(Vec::<(u32, u32)>::new());
            let mut release_resource = |slot, value, record: &mut [u8; RECORD_SIZE]| {
                assert_eq!(slot, target);
                assert_eq!(value, 0x591318 + (slot * RECORD_SIZE) as u32 + 0x4c);
                assert_ne!(
                    u32::from_le_bytes(record[0x4c..0x50].try_into().unwrap()),
                    value
                );
                events.borrow_mut().push((1, value));
                Ok(())
            };
            let mut release_model = |slot, value, record: &mut [u8; RECORD_SIZE]| {
                assert_eq!(
                    u32::from_le_bytes(record[0x7c..0x80].try_into().unwrap()),
                    value
                );
                assert_eq!(slot, target);
                events.borrow_mut().push((2, value));
                Ok(())
            };
            let mut refresh = || {
                events.borrow_mut().push((4, 0));
                Ok(())
            };
            let mut callbacks = RemovalCallbacks {
                release_resource: &mut release_resource,
                release_model: &mut release_model,
                refresh: &mut refresh,
            };
            let mut globals = RemovalGlobals {
                global_590f60: &mut global_590f60,
                global_6175f8: &mut global_6175f8,
            };
            let actual = remove_category_building(
                &mut category,
                &mut records,
                target_id,
                0x591318,
                &mut globals,
                &mut geometry,
                &mut callbacks,
            )
            .unwrap();
            let events = events.into_inner();
            assert!(
                return_reached && !fault,
                "native case {case} did not return"
            );
            assert_eq!(actual, (result != 0).then_some(target));
            assert_eq!(
                events,
                native_calls
                    .iter()
                    .filter_map(|call| {
                        (call.2 != 3).then_some((call.2, call.3.first().copied().unwrap_or(0)))
                    })
                    .collect::<Vec<_>>()
            );
            // The category and every captured selected record are the complete
            // before/after state, including untouched bytes.
            assert_eq!(
                category.as_slice(),
                expected_category.as_slice(),
                "category case {case}"
            );
            assert_eq!(
                u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap()),
                expected_head
            );
            assert_eq!(
                u32::from_le_bytes(category[0x160..0x164].try_into().unwrap()),
                expected_tail
            );
            assert_eq!(
                u32::from_le_bytes(category[8..12].try_into().unwrap()),
                expected_count
            );
            assert_eq!(global_590f60, expected_590f60);
            assert_eq!(global_6175f8, expected_6175f8);
            for (slot, expected) in expected_records {
                assert_eq!(
                    records[slot].as_slice(),
                    expected.as_slice(),
                    "building case {case}, slot {slot}"
                );
            }
            let _ = (category_index, global_target, selected, before_globals);
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original0047e450 cleanup observations"]
    fn matches_native_auxiliary_cleanup() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUILDING_REMOVE_CLEANUP")).unwrap();
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        let blob = |at: &mut usize| {
            let len = word(at) as usize;
            let value = bytes[*at..*at + len].to_vec();
            *at += len;
            value
        };
        assert_eq!(&bytes[..9], b"MTBCLEAN1");
        let mut at = 9;
        assert_eq!(word(&mut at), 1);
        let cases = word(&mut at);
        for case in 0..cases {
            let _name = blob(&mut at);
            let active_word = word(&mut at);
            let active = active_word == 1;
            for _ in 0..3 {
                let count = word(&mut at) as usize;
                at += count * 4;
            }
            for _ in 0..2 {
                let count = word(&mut at) as usize;
                at += count * 4;
            }
            let before = blob(&mut at);
            let expected_aux = blob(&mut at);
            assert_eq!(before.len(), AUXILIARY_RECORD_SIZE);
            assert_eq!(expected_aux.len(), AUXILIARY_RECORD_SIZE);
            let expected_renderer_handle = word(&mut at);
            let expected_renderer_active = word(&mut at);
            let expected_renderer_count = word(&mut at);
            let expected_registry_active = word(&mut at);
            let expected_registry_count = word(&mut at);
            let return_reached = word(&mut at) != 0;
            let fault = word(&mut at) != 0;
            let call_count = word(&mut at) as usize;
            let mut native_calls = Vec::with_capacity(call_count);
            for _ in 0..call_count {
                let _address = word(&mut at);
                let _ecx = word(&mut at);
                let kind = word(&mut at);
                let argc = word(&mut at) as usize;
                let args: Vec<u32> = (0..argc).map(|_| word(&mut at)).collect();
                native_calls.push((kind, args));
            }
            let mut aux: [u8; AUXILIARY_RECORD_SIZE] = before.try_into().unwrap();
            let events = RefCell::new(Vec::<(u32, u32)>::new());
            let mut renderer_handle = 40_u32;
            let mut renderer_active = true;
            let mut renderer_count = 9_u32;
            let mut registry_active = true;
            let mut registry_count = 7_u32;
            let mut release_array = |array, index, value| {
                let _array = array;
                events.borrow_mut().push((1, value));
                assert!(index < 2);
                Ok(())
            };
            let mut release_renderer = |index| {
                assert_eq!(index, 0);
                if renderer_active && renderer_handle != 0 {
                    events.borrow_mut().push((1, renderer_handle));
                    renderer_handle = 0;
                }
                renderer_active = false;
                renderer_count = renderer_count.wrapping_sub(1);
                Ok(())
            };
            let mut release_aux_registry = |index| {
                assert_eq!(index, 0);
                if registry_active {
                    registry_active = false;
                    registry_count = registry_count.wrapping_sub(1);
                }
                Ok(())
            };
            let mut refresh = |_record: &mut [u8; AUXILIARY_RECORD_SIZE]| {
                events.borrow_mut().push((2, 0));
                Ok(())
            };
            let mut callbacks = AuxiliaryCleanupCallbacks {
                release_array: &mut release_array,
                release_renderer: &mut release_renderer,
                release_aux_registry: &mut release_aux_registry,
                refresh: &mut refresh,
            };
            let result = cleanup_auxiliary_record(&mut aux, &mut callbacks).unwrap();
            assert_eq!(result, active);
            assert_eq!(aux.as_slice(), expected_aux.as_slice(), "aux case {case}");
            assert_eq!(renderer_handle, expected_renderer_handle);
            assert_eq!(renderer_active as u32, expected_renderer_active);
            assert_eq!(renderer_count, expected_renderer_count);
            assert_eq!(registry_active as u32, expected_registry_active);
            assert_eq!(registry_count, expected_registry_count);
            assert!(
                return_reached && !fault,
                "native case {case} did not return"
            );
            let expected_events = native_calls
                .iter()
                .map(|(kind, args)| (*kind, if *kind == 2 { 0 } else { args[0] }))
                .collect::<Vec<_>>();
            assert_eq!(
                events.into_inner(),
                expected_events,
                "call order case {case}"
            );
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original4450e0 constructor observations"]
    fn matches_native_building_constructor() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUILDING_CONSTRUCTOR")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTBCONS1");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let input = Initialization {
                rectangle: std::array::from_fn(|i| word(at + i * 4) as i32),
                height: word(at + 16),
                center: [f32::from_bits(word(at + 20)), f32::from_bits(word(at + 24))],
                variation: word(at + 28),
                kind: word(at + 32),
                quality: word(at + 36) as i8,
                total_cost: word(at + 40),
                date: [word(at + 44) as u8, word(at + 48) as u8],
            };
            at += 52;
            let mut record: [u8; RECORD_SIZE] = bytes[at..at + RECORD_SIZE].try_into().unwrap();
            at += RECORD_SIZE;
            initialize(&mut record, &input, &image).unwrap();
            for (offset, (actual, expected)) in
                record.iter().zip(&bytes[at..at + RECORD_SIZE]).enumerate()
            {
                assert_eq!(actual, expected, "case {case}, offset {offset:x}");
            }
            at += RECORD_SIZE;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original building-pool helper observations"]
    fn matches_native_building_pool() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUILDING_POOL")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTBPOOL1");
        assert_eq!(word(8), 1);
        let mut at = 28;
        for _ in 0..word(12) {
            assert_eq!(word(at), 0x100f0000);
            let mut temporary = [0; RECORD_SIZE];
            temporary[..4].copy_from_slice(&word(at + 4).to_le_bytes());
            temporary[0x3f] = word(at + 8) as u8;
            let expected_temporary = [word(at + 12), word(at + 16)];
            let before_len = word(at + 20) as usize;
            let after_len = word(at + 24) as usize;
            at += 28;
            let mut records: Vec<[u8; RECORD_SIZE]> = bytes[at..at + before_len]
                .chunks_exact(RECORD_SIZE)
                .map(|row| row.try_into().unwrap())
                .collect();
            at += before_len;
            reset(&mut records, &mut temporary).unwrap();
            assert_eq!(temporary[..4], expected_temporary[0].to_le_bytes());
            assert_eq!(temporary[0x3f], expected_temporary[1] as u8);
            assert_eq!(after_len, COUNT * RECORD_SIZE);
            for (index, record) in records.iter().enumerate() {
                assert_eq!(
                    record.as_slice(),
                    &bytes[at + index * RECORD_SIZE..at + (index + 1) * RECORD_SIZE]
                );
            }
            at += after_len;
        }
        for _ in 0..word(16) {
            let expected = word(at + 8);
            assert_eq!(word(at + 12), 0x100f0000);
            let len = word(at + 20) as usize;
            assert_eq!(len, COUNT);
            at += 24;
            let mut records = vec![[0; RECORD_SIZE]; COUNT];
            for (record, active) in records.iter_mut().zip(&bytes[at..at + len]) {
                record[0x3f] = *active;
            }
            assert_eq!(
                first_free(&records).unwrap().map_or(u32::MAX, |i| i as u32),
                expected
            );
            assert_eq!(&bytes[at..at + len], &bytes[at + len..at + len * 2]);
            at += len * 2;
        }
        for _ in 0..word(20) {
            let rectangle = std::array::from_fn(|i| word(at + i * 4) as i32);
            let height = word(at + 16) as u8;
            let apply = bytes[at + 20] != 0;
            let expected_before = word(at + 21) as u8 != 0;
            let expected_after = word(at + 25) as u8 != 0;
            assert_eq!(word(at + 29), 0x100f0000);
            at += 33;
            let mut matrix: [u8; 49] = bytes[at..at + 49].try_into().unwrap();
            assert_eq!(can_place(&matrix, rectangle), expected_before);
            if apply {
                fill_occupancy(&mut matrix, rectangle, height).unwrap();
                assert_eq!(can_place(&matrix, rectangle), expected_after);
            }
            assert_eq!(matrix, bytes[at + 49..at + 98]);
            at += 98;
        }
        for case in 0..word(24) {
            let mut record = [0; RECORD_SIZE];
            record[4..9].copy_from_slice(&bytes[at..at + 5]);
            assert_eq!(
                capacity(&record, f32::from_bits(word(at + 9))),
                word(at + 13) as i32,
                "capacity case {case}"
            );
            assert_eq!(word(at + 17), 0x100f0000);
            at += 21;
        }
        assert_eq!(at, bytes.len());
    }
}
