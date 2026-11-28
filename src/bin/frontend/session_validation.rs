//! Independent Host-level checks for the actual result modal and campaign record.
//! These are controlled outcome injections, not natural competitive playthroughs.

/// The original 0x48c binding writes the reciprocal business/building indexes.
/// Run this during natural Host hours so later construction and sale can expose
/// a stale link even when money and the clock continue to change.
pub fn assert_live_links(session: &crate::fresh_city::Session<'_>) {
    for (slot, business) in session.world.ledgers.businesses.iter().enumerate() {
        if business[0x21c] == 0 {
            continue;
        }
        let building = i16::from_le_bytes(business[8..10].try_into().unwrap());
        let record = session
            .world
            .buildings
            .get(building as usize)
            .unwrap_or_else(|| panic!("active business {slot} has missing building {building}"));
        let reciprocal = u32::from_le_bytes(record[0x1c..0x20].try_into().unwrap());
        assert_eq!(
            reciprocal, slot as u32,
            "active business {slot} has stale building {building} reciprocal link"
        );
        let owner = usize::from(business[10]);
        assert!(
            owner <= session.roster.manager.players.len(),
            "active business {slot} owner leaves roster"
        );
    }
}

#[cfg(test)]
#[path = "session_validation_tests.rs"]
mod tests;
