//! Worker-side by-value producer, called before camera Worker::request.
//! Host receives these values without an event-thread Session lookup.
use super::selection::SelectedBuilding;

pub fn selected_business(session: &crate::fresh_city::Session<'_>, target: u32)
    -> Result<SelectedBuilding, String> {
    let business = session.world.ledgers.businesses.get(target as usize)
        .ok_or("Camera.SetMode13 business leaves current pool")?;
    if business[0x21c] == 0 { return Err("Camera.SetMode13 business is not live".into()); }
    let slot = i16::from_le_bytes(business[8..10].try_into().unwrap());
    let slot = usize::try_from(slot).map_err(|_| "Camera.SetMode13 building link is negative")?;
    let building = session.world.buildings.get(slot)
        .ok_or("Camera.SetMode13 building leaves current pool")?;
    let word = |at| u32::from_le_bytes(building[at..at + 4].try_into().unwrap());
    if word(0x1c) != target { return Err("Camera.SetMode13 live reciprocal link disagrees".into()); }
    let offset = word(0x40).checked_sub(openmt2001::building_geometry::V14_CATEGORY_BASE)
        .ok_or("Camera.SetMode13 category precedes arena")?;
    if offset % 0x17c != 0 || offset / 0x17c >= 52 {
        return Err("Camera.SetMode13 category leaves arena".into());
    }
    let category = (offset / 0x17c) as usize;
    let chains = openmt2001::building_pool::category_chains(
        &session.world.categories, &session.world.buildings, 0x591318,
    )?;
    if !chains[category].contains(&slot) { return Err("Camera.SetMode13 building is no longer linked".into()); }
    let category_id = u32::from_le_bytes(session.world.categories[category][..4].try_into().unwrap());
    Ok(SelectedBuilding {
        business_slot: target, building_slot: slot as u32, category_id,
        selected_xz_words: [word(0x0c), word(0x10)], building_kind: word(0x24),
        // Native reads these raw slots when its auto/non-landscape branch
        // reaches them. Do not require count>0 or use Scene's Option entrance.
        entrance_words: std::array::from_fn(|i| word(0x224 + i * 4)),
    })
}
