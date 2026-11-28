//! Original0047cc60 residential rent decisions and conditional submission.

use crate::economy::{
    BusinessDayInputs, LocalBusinessCommands, PendingBusinessTerms, Utilities, ftol_low_word,
};

pub struct Inputs<'a, 'b> {
    pub business: &'a BusinessDayInputs<'b>,
    pub utilities: &'a Utilities,
    /// Integer result from original47d9f0, after its native conversion.
    pub increase: i32,
}

impl<'a, 'b> Inputs<'a, 'b> {
    pub fn from_profile(
        business: &'a BusinessDayInputs<'b>,
        utilities: &'a Utilities,
        profile: &[u8; 0x650],
        difficulty: i32,
    ) -> Result<Self, String> {
        Ok(Self {
            business,
            utilities,
            increase: crate::ai_parameters::rent_increase(profile, difficulty)?,
        })
    }
}

/// Snapshot residential candidates before adjusting any settings. Duplicate
/// ownership entries remain duplicate visits, as in original46b270.
pub fn adjust_owned(
    list: &crate::ai_ownership::BusinessList,
    businesses: &mut [[u8; 544]],
    inputs: &Inputs<'_, '_>,
) -> Result<bool, String> {
    let mut candidates = Vec::new();
    //46b270 scans allocation capacity at b46, including holes past b44.
    for entry in &list.entries {
        if entry[0] == 0 {
            continue;
        }
        let index = u32::from_le_bytes(entry[4..8].try_into().unwrap()) as usize;
        let record = businesses
            .get(index)
            .ok_or("AI rental business leaves pool")?;
        if record[..4] == 1_u32.to_le_bytes() {
            if candidates.len() == 100 {
                return Err("original AI rental scratch list exceeds100 entries".into());
            }
            candidates.push(index);
        }
    }
    let mut changed = false;
    for index in candidates {
        let record = &mut businesses[index];
        let capacity = i32::from(u16::from_le_bytes(record[14..16].try_into().unwrap()));
        let occupied = i32::from(u16::from_le_bytes(record[16..18].try_into().unwrap()));
        let vacant = capacity - occupied;
        let old = ftol_low_word(f64::from(f32::from_le_bytes(
            record[0x1dc..0x1e0].try_into().unwrap(),
        )));
        let cost =
            crate::ai_business::estimate_cost_extended(record, inputs.business, inputs.utilities)?
                .ftol_low_word();
        let minimum = cost
            .checked_div(capacity)
            .ok_or("original AI rent divides by zero capacity")?;
        let next = if vacant > 0 && minimum < old {
            ftol_low_word(f64::from(old) - (f64::from(vacant) / f64::from(capacity)) * 5.0)
                .max(minimum)
        } else if vacant == 0 {
            old.wrapping_add(inputs.increase)
        } else {
            continue;
        };
        let next = if next <= 0 { 0.1_f32 } else { next as f32 };
        record[0x204..0x208].copy_from_slice(&next.to_le_bytes());
        record[0x208] = 1;
        changed = true;
    }
    Ok(changed)
}

pub fn update(
    list: &crate::ai_ownership::BusinessList,
    businesses: &mut [[u8; 544]],
    inputs: &Inputs<'_, '_>,
    player: u8,
    tick: u32,
    commands: &mut LocalBusinessCommands,
    apply: impl FnMut(&PendingBusinessTerms, &mut [[u8; 544]]) -> Result<(), String>,
) -> Result<(), String> {
    if adjust_owned(list, businesses, inputs)? {
        commands.submit_pending(businesses, i32::from(player), tick, apply)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original rent decision observations"]
    fn matches_native_rent_decisions() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_RENT")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAIRNT2");
        assert_eq!(word(12), 0x037f);
        let mut at = 16;
        for _ in 0..word(8) {
            let name_len = word(at) as usize;
            let player = word(at + 4) as u8;
            let count = word(at + 8) as usize;
            assert_eq!(word(at + 12), 600);
            at += 16;
            let name = std::str::from_utf8(&bytes[at..at + name_len]).unwrap();
            at += name_len;
            let list = crate::ai_ownership::BusinessList {
                entries: bytes[at..at + count * 28]
                    .chunks_exact(28)
                    .map(|r| r.try_into().unwrap())
                    .collect(),
                active_count: 0, // The native traversal is capacity-based, independent of this counter.
            };
            at += count * 28;
            let mut buildings = [[0; 64]; 2];
            for building in &mut buildings {
                for i in 0..5 {
                    building[4 + i] = word(at + i * 4) as u8;
                }
                at += 32;
            }
            let mut settings = crate::economy::BusinessDaySettings {
                capacity_rates: [0.; 45],
                utility_rates: [[0.; 4]; 45],
                catalog_masks: [0; 45],
            };
            for kind in 0..2 {
                settings.utility_rates[kind] =
                    std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
                at += 16;
            }
            let utilities = Utilities {
                owners: bytes[at..at + 4].try_into().unwrap(),
                enabled: std::array::from_fn(|i| bytes[at + 4 + i] != 0),
                totals: [0.; 4],
            };
            at += 8;
            assert_eq!([word(at), word(at + 4)], [600, 544]);
            at += 8;
            let mut businesses: Vec<[u8; 544]> = bytes[at..at + 600 * 544]
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 600 * 544;
            let expected = &bytes[at..at + 600 * 544];
            at += 600 * 544;
            let increase = f64::from_le_bytes(bytes[at + 8..at + 16].try_into().unwrap()) as f32;
            at += 16;
            let submitted = word(at) as usize;
            at += 4;
            let expected_payloads = &bytes[at..at + submitted * 44];
            at += submitted * 44;
            // Native helper traces are retained for diagnosis in the fixture.
            let costs = word(at) as usize;
            at += 4 + costs * 8;
            let increments = word(at) as usize;
            at += 4 + increments * 8;
            let flushes = word(at) as usize;
            at += 4 + flushes * 4;
            let catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| vec![]),
                fractions: std::array::from_fn(|_| vec![]),
            };
            let business = BusinessDayInputs {
                buildings: &buildings,
                building_categories: &[],
                category_owners: &[0; 52],
                catalogs: &catalogs,
                unit_prices: &[0.; 32],
                settings: &settings,
            };
            let mut profile = [0; 0x650];
            let mut parameters = [0.; 16];
            parameters[4] = f64::from(increase);
            crate::ai_parameters::set_difficulty(&mut profile, parameters).unwrap();
            let inputs = Inputs::from_profile(&business, &utilities, &profile, 0).unwrap();
            let mut commands = LocalBusinessCommands::default();
            // Feed the rent stage through the original timetable setter,
            // primary queue and producer rather than injecting its request bit.
            crate::ai_events::configure_timetable(&mut profile, true, [1., 0., 0., 0.]).unwrap();
            crate::ai_events::configure_timetable(&mut profile, false, [1., 0., 0., 1.]).unwrap();
            crate::ai_events::configure_timetable(&mut profile, true, [2., 0., 0., 0.]).unwrap();
            crate::ai_events::configure_timetable(&mut profile, false, [2., 0., 0., 7.]).unwrap();
            let mut queue = crate::ai_events::Timetable::from_profile(&profile, 1);
            let mut secondary = crate::ai_events::Timetable::secondary_from_profile(&profile, 1);
            let mut scheduled = crate::ai_events::ScheduledEvents {
                active: 1,
                primary: crate::ai_events::State {
                    category: -1,
                    ..Default::default()
                },
                ..Default::default()
            };
            crate::ai_events::poll(&mut scheduled, &mut queue, &mut secondary, 0).unwrap();
            assert_eq!(scheduled.primary.pending, 1);
            assert_eq!(scheduled.secondary.pending, 1);
            crate::ai_events::consume_secondary(&mut scheduled, &mut secondary, 0, &[], |_, _| {
                panic!("recognized secondary event unexpectedly logged")
            })
            .unwrap();
            assert_eq!(scheduled.secondary.pending, 0);
            let event_state = &mut scheduled.primary;
            crate::ai_events::consume(
                event_state,
                0,
                &[],
                |_, _| panic!("rent event unexpectedly logged"),
                |_| queue.advance().map(Some),
            )
            .unwrap();
            assert_eq!(queue.enabled, 0);
            assert_eq!(event_state.pending, 0);
            let mut flags = event_state.flags;
            crate::ai_dispatch::advance(&mut flags, &mut false, |event, _, _, _| {
                assert_eq!(event, crate::ai_dispatch::Event::RentSettings);
                update(
                    &list,
                    &mut businesses,
                    &inputs,
                    player,
                    0,
                    &mut commands,
                    |_, _| panic!("rent applied before delay"),
                )?;
                Ok(0)
            })
            .unwrap();
            assert_eq!(flags, 0);
            let differences: Vec<_> = businesses
                .as_flattened()
                .iter()
                .zip(expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(at, (a, b))| (at / 544, at % 544, *a, *b))
                .take(20)
                .collect();
            assert!(differences.is_empty(), "{name}: {differences:?}");
            commands
                .drain(&mut [24; 2], |_| panic!("rent delivered early"))
                .unwrap();
            let mut payloads = Vec::new();
            commands
                .drain(&mut [25; 2], |command| {
                    payloads.extend(command.business_index.to_le_bytes());
                    for target in command.terms.stock_targets {
                        payloads.extend(target.to_le_bytes());
                    }
                    for price in command.terms.prices {
                        payloads.extend(price.to_le_bytes());
                    }
                    payloads.extend(command.terms.rent.to_le_bytes());
                    Ok(())
                })
                .unwrap();
            assert_eq!(payloads, expected_payloads, "{name}");
        }
        assert_eq!(at, bytes.len());
    }
}
