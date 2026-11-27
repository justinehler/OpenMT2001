//! Original adjustment-arrow writes and submission over the retained city.
use crate::fresh_city::{CommandAssets, Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Refresh,
    Price { slot: usize, increase: bool },
    Stock { slot: usize, increase: bool },
    Rent { increase: bool },
    Apply,
}
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub building: usize,
    pub business: usize,
}

/// Original419cfb/419d3c and40cb68/40cbd5. Retail compares the extended
/// result; apartments first store the result as float. Both clamp to0.1.
fn monetary_step(record: &mut [u8; 544], at: usize, increase: bool, apartment: bool) {
    let current = f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    let mut value = f64::from(current)
        + if increase {
            f64::from(0.1_f32)
        } else {
            -f64::from(0.1_f32)
        };
    if apartment {
        value = f64::from(value as f32);
    }
    let minimum = f64::from(0.1_f32);
    // FCOM sets its less-than bit for unordered as well.
    if value.partial_cmp(&minimum) != Some(std::cmp::Ordering::Greater) && value != minimum {
        value = minimum;
    }
    record[at..at + 4].copy_from_slice(&(value as f32).to_le_bytes());
    record[0x208] = 1;
}

pub fn edit(
    session: &mut Session<'_>,
    assets: &CommandAssets<'_, '_>,
    target: Target,
    action: Action,
) -> Result<[u8; 544], String> {
    let building = session
        .world
        .buildings
        .get(target.building)
        .ok_or("adjustment building leaves city")?;
    let record = session
        .world
        .ledgers
        .businesses
        .get(target.business)
        .ok_or("adjustment business leaves city")?;
    let player = session
        .roster
        .manager
        .players
        .iter()
        .find(|player| player.reference == session.roster.status.current_player)
        .map(|player| i32::from(player.bytes[0x1bc]))
        .ok_or("Current player is not in the live roster")?;
    if building[0x3f] == 0
        || record[0x21c] == 0
        || u32::from_le_bytes(building[0x1c..0x20].try_into().unwrap()) as usize != target.business
        || usize::from(u16::from_le_bytes(record[8..10].try_into().unwrap())) != target.building
        || i32::from(record[0xa]) != player + 1
    {
        return Err("Business is no longer active and owned by the current player".into());
    }
    let mut draft = *record;
    let kind = u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize;
    let info = session
        .world
        .catalog
        .info
        .row(kind)
        .ok_or("adjustment catalog leaves city")?;
    match action {
        Action::Refresh => return Ok(draft),
        Action::Price { slot, increase } | Action::Stock { slot, increase } => {
            if record[..4] != [0; 4] || slot >= usize::from(info[0x4c]) || slot >= 6 {
                return Err("Adjustment target is not a retail product".into());
            }
            if matches!(action, Action::Price { .. }) {
                monetary_step(&mut draft, 0x1ec + slot * 4, increase, false);
            } else {
                let at = 0x1e0 + slot * 2;
                let old = i16::from_le_bytes(record[at..at + 2].try_into().unwrap());
                let quantity = if increase {
                    old.wrapping_add(1)
                } else {
                    old.wrapping_sub(1)
                };
                let catalog = session.world.catalog.products[kind].map(|p| p[0]);
                openmt2001::economy::adjust_stock_target(
                    &mut draft,
                    building[..64].try_into().unwrap(),
                    f32::from_le_bytes(info[0x58..0x5c].try_into().unwrap()),
                    &catalog,
                    f32::from_bits(session.world.catalog.products[kind][slot][1]),
                    slot,
                    quantity,
                )?;
            }
        }
        Action::Rent { increase } => {
            if record[..4] != [1, 0, 0, 0] {
                return Err("Rent adjustment requires an apartment".into());
            }
            monetary_step(&mut draft, 0x204, increase, true);
        }
        Action::Apply => {
            // Original464060 clears dirty before scheduling all of this player's
            // pending records. The normal queue applies their terms25 ticks later.
            let mut commands = Vec::new();
            openmt2001::economy::submit_business_terms(
                &mut session.world.ledgers.businesses,
                player,
                |command, _| {
                    commands.push(command);
                    Ok(())
                },
            )?;
            for command in commands {
                session.submit_command(
                    openmt2001::live_commands::Command::Terms(command),
                    assets,
                    |event| Err(format!("Immediate adjustment callback requires {event:?}")),
                )?;
            }
            return Ok(session.world.ledgers.businesses[target.business]);
        }
    }
    session.world.ledgers.businesses[target.business] = draft;
    Ok(draft)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original business adjustment arrow fixture"]
    fn matches_native_monetary_arrows() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_BUSINESS_EDIT")).unwrap();
        assert_eq!(&bytes[..8], b"MTBEDT01");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 12 + count * 1092);
        for (i, case) in bytes[12..].chunks_exact(1092).enumerate() {
            let apartment = case[0] != 0;
            let increase = case[1] != 0;
            let slot = usize::from(case[2]);
            let mut record: [u8; 544] = case[4..548].try_into().unwrap();
            monetary_step(
                &mut record,
                if apartment { 0x204 } else { 0x1ec + slot * 4 },
                increase,
                apartment,
            );
            assert_eq!(record.as_slice(), &case[548..], "case {i}");
        }
    }
}
