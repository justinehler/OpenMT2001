//! Original00467f40 retail stock/price decisions over shared business records.

use crate::economy::{BusinessDayInputs, ftol_low_word, stock_capacity};

///472620: signed-positive category/type counts decrement; zero and negatives remain.
pub fn remove_category_count(
    counts: &mut [u32; 52 * 45],
    category: u32,
    kind: u32,
) -> Result<(), String> {
    let index = category.wrapping_mul(45).wrapping_add(kind) as usize;
    let count = counts
        .get_mut(index)
        .ok_or("removed business count leaves table")?;
    if (*count as i32) > 0 {
        *count -= 1;
    }
    Ok(())
}

/// Original472520: rebuild the flattened52-by45 table from category chains.
/// No active, owner or discriminator filtering occurs. A type can spill into
/// the next row as in the original, but writes outside the table are rejected.
pub fn rebuild_category_counts<const N: usize>(
    counts: &mut [u32; 52 * 45],
    chains: &[Vec<usize>; 52],
    buildings: &[[u8; N]],
    businesses: &[[u8; 544]],
) -> Result<(), String> {
    if N < 0x20 {
        return Err("AI count building record lacks its business link".into());
    }
    counts.fill(0);
    for (category, chain) in chains.iter().enumerate() {
        for &slot in chain {
            let building = buildings.get(slot).ok_or("AI count building leaves pool")?;
            let index = i32::from_le_bytes(building[0x1c..0x20].try_into().unwrap());
            if index < 0 {
                continue;
            }
            let business = businesses
                .get(index as usize)
                .ok_or("AI count business leaves pool")?;
            let kind = u32::from_le_bytes(business[4..8].try_into().unwrap());
            let index = (category as u32 * 45).wrapping_add(kind) as usize;
            let count = counts
                .get_mut(index)
                .ok_or("AI count leaves flattened table")?;
            *count = count.wrapping_add(1);
        }
    }
    Ok(())
}

fn short(record: &[u8; 544], at: usize) -> i16 {
    i16::from_le_bytes(record[at..at + 2].try_into().unwrap())
}

fn float(record: &[u8; 544], at: usize) -> f32 {
    f32::from_le_bytes(record[at..at + 4].try_into().unwrap())
}

fn below_or_unordered(left: f64, right: f64) -> bool {
    left < right || left.is_nan() || right.is_nan()
}

// The original takes integer absolute values, stores the integral part as
// float32, and truncates the remaining tenths. This is not decimal rounding.
fn price_change(value: f64) -> f64 {
    let whole = f64::from(ftol_low_word(value).wrapping_abs() as f32);
    let tenths = ftol_low_word((value - whole) * 10.0).wrapping_abs();
    f64::from(tenths) * f64::from(0.1_f32) + whole
}

/// Original4720f0 cost estimate. Unlike actual utility assessment, this is
/// read-only and keeps utility products/sums in x87 precision until return.
pub fn estimate_cost(
    record: &[u8; 544],
    inputs: &BusinessDayInputs<'_>,
    utilities: &crate::economy::Utilities,
) -> Result<f64, String> {
    Ok(estimate_cost_extended(record, inputs, utilities)?.to_f64())
}

pub fn estimate_cost_extended(
    record: &[u8; 544],
    inputs: &BusinessDayInputs<'_>,
    utilities: &crate::economy::Utilities,
) -> Result<crate::x87::Sum, String> {
    let kind = u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize;
    let building = inputs
        .buildings
        .get(short(record, 8) as usize)
        .ok_or("AI cost building leaves pool")?;
    let scalar = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
        .wrapping_mul(i32::from(building[8]));
    let rates = inputs
        .settings
        .utility_rates
        .get(kind)
        .ok_or("AI cost type leaves settings")?;
    let mut products = 0_f32;
    if record[..4] == [0; 4] {
        let catalog = inputs
            .catalogs
            .commodities
            .get(kind)
            .ok_or("AI cost type leaves catalog")?;
        if catalog.len() > 6 {
            return Err("AI cost catalog exceeds six slots".into());
        }
        for (slot, &commodity) in catalog.iter().enumerate() {
            let stock = i32::from(short(record, 0x80 + slot * 0x34) as u16);
            let target = i32::from(short(record, 0x1b8 + slot * 2));
            if stock < target {
                let price = *inputs
                    .unit_prices
                    .get(commodity as usize)
                    .ok_or("AI cost commodity leaves table")?;
                let mut added = crate::x87::Sum::from_f32(price)?;
                added.mul(crate::x87::Sum::from_i32(target - stock));
                added.add_f32(products)?;
                products = added.to_f32();
            }
        }
    }
    let mut total = crate::x87::Sum::from_i32(0);
    for (service, rate) in rates.iter().enumerate() {
        if utilities.enabled[service] && utilities.owners[service] != record[10] {
            let mut amount = crate::x87::Sum::from_i32(scalar);
            amount.mul_f32(*rate)?;
            total.add(amount);
        }
    }
    total.add(crate::x87::Sum::from_i32(i32::from(
        short(record, 0x18) as u16
    )));
    total.add_f32(products)?;
    Ok(total)
}

/// Update one business in original stocked-first, empty-second catalog order.
/// Only pending settings and the dirty flag change; submit through the local
/// business-command path after the complete AI-owned list has been visited.
pub fn adjust(record: &mut [u8; 544], inputs: &BusinessDayInputs<'_>) -> Result<(), String> {
    let kind = u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize;
    let building = inputs
        .buildings
        .get(short(record, 8) as usize)
        .ok_or("AI business building leaves pool")?;
    let rate = *inputs
        .settings
        .capacity_rates
        .get(kind)
        .ok_or("AI business type leaves settings")?;
    let catalog = inputs
        .catalogs
        .commodities
        .get(kind)
        .ok_or("AI business type leaves catalog")?;
    if catalog.len() > 6 {
        return Err("AI business catalog exceeds six slots".into());
    }
    let area = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1);
    let capacity = stock_capacity(area, building[8], rate);
    let mut stocked = Vec::new();
    let mut empty = Vec::new();
    for slot in 0..catalog.len() {
        // Native inventory test is unsigned; live/pending target tests differ.
        if short(record, 0x80 + slot * 0x34) as u16 != 0 {
            stocked.push(slot);
        } else {
            empty.push(slot);
        }
    }
    let empty_count = empty.len() as i32;
    let mut assigned = 0_i32;
    for slot in stocked.into_iter().chain(empty) {
        let target_at = 0x1e0 + slot * 2;
        let price_at = 0x1ec + slot * 4;
        let quantity = short(record, 0x80 + slot * 0x34) as u16;
        if quantity != 0 {
            if short(record, 0x1b8 + slot * 2) <= 0 {
                continue;
            }
            let target = i32::from(short(record, target_at));
            let ratio = (i32::from(quantity) * 100)
                .checked_div(target)
                .ok_or("original AI stock ratio divides by zero")?;
            let ratio = (f64::from(ratio) * 0.625).min(50.0);
            let old_price = f64::from(float(record, 0x1c4 + slot * 4));
            let discount = (ratio * f64::from(0.01_f32)) * old_price;
            let next = old_price - price_change(discount);
            let next = if below_or_unordered(next, f64::from(0.1_f32)) {
                0.1
            } else {
                next as f32
            };
            record[price_at..price_at + 4].copy_from_slice(&next.to_le_bytes());
            record[0x208] = 1;
            let wholesale = *inputs
                .unit_prices
                .get(catalog[slot] as usize)
                .ok_or("AI business commodity leaves price table")?;
            if below_or_unordered(f64::from(next), f64::from(wholesale)) {
                let floor = if below_or_unordered(f64::from(wholesale), f64::from(0.1_f32)) {
                    0.1
                } else {
                    wholesale
                };
                record[price_at..price_at + 4].copy_from_slice(&floor.to_le_bytes());
            }
            let reduction = (f64::from(capacity) * f64::from(0.1_f32)) as f32;
            let requested =
                ftol_low_word(f64::from(short(record, target_at)) - f64::from(reduction));
            inputs.adjust_stock_target(record, slot, requested as i16)?;
            let current = short(record, target_at);
            if current < 0 {
                inputs.adjust_stock_target(record, slot, 0)?;
            } else {
                assigned = ftol_low_word(
                    (f64::from(current) - f64::from(reduction)) + f64::from(assigned),
                );
            }
        } else {
            let share = capacity
                .wrapping_sub(assigned)
                .checked_div(empty_count)
                .ok_or("original AI empty-stock division is invalid")?;
            let growth = f64::from(share) * f64::from(0.1_f32);
            let requested = if growth < 5.0 {
                short(record, target_at).wrapping_add(5)
            } else {
                ftol_low_word(f64::from(short(record, target_at)) + growth) as i16
            };
            inputs.adjust_stock_target(record, slot, requested)?;
            if i32::from(short(record, target_at)) > share {
                inputs.adjust_stock_target(record, slot, share as i16)?;
            }
            let price = f64::from(float(record, price_at));
            let change = price * f64::from(0.1_f32);
            let next = if change > 1.0 {
                price + price_change(change)
            } else {
                price + 1.0
            };
            let next = if below_or_unordered(next, f64::from(0.1_f32)) {
                0.1
            } else {
                next as f32
            };
            record[price_at..price_at + 4].copy_from_slice(&next.to_le_bytes());
            record[0x208] = 1;
        }
    }
    Ok(())
}

/// The original AI-owned entry is28 bytes; byte0 enables it and dword4 selects
/// the business. Entry order is authoritative, including repeated identities.
pub fn adjust_owned(
    list: &crate::ai_ownership::BusinessList,
    businesses: &mut [[u8; 544]],
    inputs: &BusinessDayInputs<'_>,
) -> Result<(), String> {
    // Native player+b46 is allocated capacity; b44 is the number of live
    // entries and does not delimit the scan when there are deleted holes.
    for entry in &list.entries {
        if entry[0] == 0 {
            continue;
        }
        let index = u32::from_le_bytes(entry[4..8].try_into().unwrap()) as usize;
        adjust(
            businesses
                .get_mut(index)
                .ok_or("AI-owned business leaves pool")?,
            inputs,
        )?;
    }
    Ok(())
}

/// Complete467f40's decision pass and final464060 submission. The player byte
/// is zero-extended at the native call site, unlike the signed business owner.
pub fn update(
    list: &crate::ai_ownership::BusinessList,
    businesses: &mut [[u8; 544]],
    inputs: &BusinessDayInputs<'_>,
    player: u8,
    tick: u32,
    commands: &mut crate::economy::LocalBusinessCommands,
    apply: impl FnMut(&crate::economy::PendingBusinessTerms, &mut [[u8; 544]]) -> Result<(), String>,
) -> Result<(), String> {
    adjust_owned(list, businesses, inputs)?;
    commands.submit_pending(businesses, i32::from(player), tick, apply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original472520 category count observations"]
    fn matches_native_category_counts() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_CATEGORY_COUNTS").unwrap()).unwrap();
        let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(&data[..8], b"MTAICCAT");
        assert_eq!(word(8), 1);
        assert_eq!([word(16), word(20), word(24)], [52, 45, 544]);
        let mut at = 28;
        for _ in 0..word(12) {
            let name_length = word(at) as usize;
            let nodes = word(at + 4) as usize;
            at += 8 + name_length;
            let mut chains: [Vec<usize>; 52] = std::array::from_fn(|_| vec![]);
            let mut buildings = vec![[0; 64]; nodes];
            let mut businesses = vec![[0; 544]; 600];
            for (slot, building) in buildings.iter_mut().enumerate() {
                let category = word(at) as usize;
                let index = word(at + 4) as i32;
                chains[category].push(slot);
                building[0x1c..0x20].copy_from_slice(&index.to_le_bytes());
                if index >= 0 {
                    let business = &mut businesses[index as usize];
                    business[4..8].copy_from_slice(&word(at + 8).to_le_bytes());
                    business[..4].copy_from_slice(&word(at + 12).to_le_bytes());
                    business[0x21c] = word(at + 16) as u8;
                }
                at += 20;
            }
            let mut counts = std::array::from_fn(|i| word(at + i * 4));
            at += 52 * 45 * 4;
            rebuild_category_counts(&mut counts, &chains, &buildings, &businesses).unwrap();
            let live_buildings: Vec<[u8; 0x394]> = buildings
                .iter()
                .map(|prefix| {
                    let mut live = [0xa5; 0x394];
                    live[..64].copy_from_slice(prefix);
                    live
                })
                .collect();
            let mut live_counts = [u32::MAX; 52 * 45];
            rebuild_category_counts(&mut live_counts, &chains, &live_buildings, &businesses)
                .unwrap();
            assert_eq!(
                live_counts, counts,
                "live building records must use the same native count path"
            );
            for (i, count) in counts.iter().enumerate() {
                assert_eq!(*count, word(at + i * 4), "cell {i}");
            }
            at += 52 * 45 * 4;
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original business cost-estimate observations"]
    fn matches_native_cost_estimate() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_COST").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAICST1");
        let mut at = 12;
        for case in 0..word(8) {
            let record: &[u8; 544] = bytes[at..at + 544].try_into().unwrap();
            at += 544;
            let mut buildings = [[0; 64]; 2];
            buildings[1].copy_from_slice(&bytes[at..at + 64]);
            at += 64;
            let rates = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
            at += 16;
            let owners = bytes[at..at + 4].try_into().unwrap();
            let enabled = std::array::from_fn(|i| bytes[at + 4 + i] != 0);
            at += 8;
            let count = word(at) as usize;
            at += 4;
            let mut catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| vec![]),
                fractions: std::array::from_fn(|_| vec![]),
            };
            catalogs.commodities[17] = (0..count).map(|i| word(at + i * 4)).collect();
            at += 24;
            let prices = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
            at += 128;
            let expected = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
            at += 8;
            let mut settings = crate::economy::BusinessDaySettings {
                capacity_rates: [0.; 45],
                utility_rates: [[0.; 4]; 45],
                catalog_masks: [0; 45],
            };
            settings.utility_rates[17] = rates;
            let inputs = BusinessDayInputs {
                buildings: &buildings,
                building_categories: &[],
                category_owners: &[0; 52],
                catalogs: &catalogs,
                unit_prices: &prices,
                settings: &settings,
            };
            let utilities = crate::economy::Utilities {
                owners,
                enabled,
                totals: [123.5; 4],
            };
            assert_eq!(
                estimate_cost(record, &inputs, &utilities)
                    .unwrap()
                    .to_bits(),
                expected,
                "AI cost case{case}"
            );
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original AI retail decision and submission observations"]
    fn matches_native_retail_decisions() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_BUSINESS").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAIBUS2");
        assert_eq!(word(12), 0x037f);
        let mut at = 16;
        for case in 0..word(8) {
            let player = word(at) as u8;
            let count = word(at + 4) as usize;
            at += 8;
            let entries = bytes[at..at + count * 28]
                .chunks_exact(28)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += count * 28;
            let list = crate::ai_ownership::BusinessList {
                entries,
                active_count: 0, // The native traversal is capacity-based, independent of this counter.
            };
            let mut buildings = [[0_u8; 64]; 2];
            for building in &mut buildings {
                building[4] = word(at) as u8;
                building[5] = word(at + 4) as u8;
                building[6] = word(at + 8) as u8;
                building[7] = word(at + 12) as u8;
                building[8] = word(at + 16) as u8;
                at += 32;
            }
            let capacity = f32::from_bits(word(at));
            let count = word(at + 4) as usize;
            at += 8;
            let mut catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| vec![]),
                fractions: std::array::from_fn(|_| vec![]),
            };
            for slot in 0..6 {
                if slot < count {
                    catalogs.commodities[0].push(word(at));
                    catalogs.fractions[0].push(f32::from_bits(word(at + 4)));
                }
                at += 8;
            }
            let prices = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
            at += 128;
            assert_eq!([word(at), word(at + 4)], [600, 544]);
            at += 8;
            let mut businesses: Vec<[u8; 544]> = bytes[at..at + 600 * 544]
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 600 * 544;
            let expected = &bytes[at..at + 600 * 544];
            at += 600 * 544;
            let submitted = word(at) as usize;
            at += 4;
            let expected_payloads = &bytes[at..at + submitted * 44];
            at += submitted * 44;
            // The oracle additionally retains helper argument traces for
            // diagnosis; production is compared by full records and payloads.
            let adjustments = word(at) as usize;
            at += 4 + adjustments * 12;
            assert_eq!(word(at), 1);
            assert_eq!(word(at + 4), u32::from(player));
            at += 8;
            let mut settings = crate::economy::BusinessDaySettings {
                capacity_rates: [0.; 45],
                utility_rates: [[0.; 4]; 45],
                catalog_masks: [0; 45],
            };
            settings.capacity_rates[0] = capacity;
            let inputs = BusinessDayInputs {
                buildings: &buildings,
                building_categories: &[],
                category_owners: &[0; 52],
                catalogs: &catalogs,
                unit_prices: &prices,
                settings: &settings,
            };
            let mut commands = crate::economy::LocalBusinessCommands::default();
            let mut flags = 0x100;
            crate::ai_dispatch::advance(&mut flags, &mut false, |event, _, _, _| {
                assert_eq!(event, crate::ai_dispatch::Event::BusinessSettings);
                update(
                    &list,
                    &mut businesses,
                    &inputs,
                    player,
                    0,
                    &mut commands,
                    |_, _| panic!("AI settings applied before their delay"),
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
                .filter(|(_, (actual, expected))| actual != expected)
                .map(|(at, (actual, expected))| (at / 544, at % 544, *actual, *expected))
                .take(20)
                .collect();
            assert!(
                differences.is_empty(),
                "AI records case{case}: {differences:?}"
            );
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
            assert_eq!(payloads, expected_payloads, "AI submission case{case}");
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original v1.4 Scenario 3 saved retail decision observations"]
    fn matches_native_saved_retail_follow_through() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_SAVED_BUSINESS_SETTINGS").unwrap())
                .unwrap();
        let save = std::fs::read(std::env::var("OPENMT2001_SAVE_ONE").unwrap()).unwrap();
        let catalog_oracle =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CATALOG").unwrap()).unwrap();
        let word =
            |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 0;
        let take = |at: &mut usize, n: usize| {
            let result = &data[*at..*at + n];
            *at += n;
            result
        };
        assert_eq!(take(&mut at, 8), b"MTASBS01");
        assert_eq!(word(&data, at), 0x037f);
        at += 4;
        assert_eq!(word(&data, at), 600);
        at += 4;
        let owned = word(&data, at) as usize;
        at += 4;
        assert_eq!(owned, 6);
        let entries = take(&mut at, owned * 28);
        let building_prefixes = take(&mut at, 600 * 64);
        let info_bytes = take(&mut at, 45 * 116);
        let product_bytes = take(&mut at, 45 * 48);
        let commodity_bytes = take(&mut at, 32 * 28);
        let before = take(&mut at, 600 * 544);
        let expected = take(&mut at, 600 * 544);
        let submitted = word(&data, at) as usize;
        at += 4;
        let expected_payloads = take(&mut at, submitted * 44);
        let adjustments = word(&data, at) as usize;
        at += 4 + adjustments * 12;
        assert_eq!(word(&data, at), 1);
        assert_eq!(word(&data, at + 4), 0);
        at += 8;
        assert_eq!(at, data.len());
        assert_eq!((submitted, adjustments), (6, 6));

        let mut saved_at = 0;
        let mut saved_building_prefixes = None;
        let mut saved_businesses = None;
        let mut saved_info = None;
        let mut saved_commodities = None;
        loop {
            let tag = word(&save, saved_at);
            let size = word(&save, saved_at + 4) as usize;
            saved_at += 8;
            if tag == u32::MAX {
                break;
            }
            let payload = &save[saved_at..saved_at + size];
            saved_at += size;
            match tag {
                4 => saved_building_prefixes = Some(&payload[..600 * 64]),
                0x20 => saved_businesses = Some(payload),
                0x40000 => saved_info = Some(payload),
                0x200000 => saved_commodities = Some(payload),
                _ => (),
            }
        }
        assert_eq!(saved_at, save.len());
        assert_eq!(saved_building_prefixes.unwrap(), building_prefixes);
        assert_eq!(saved_businesses.unwrap(), before);
        assert_eq!(saved_info.unwrap(), info_bytes);
        assert_eq!(saved_commodities.unwrap(), commodity_bytes);

        assert_eq!(&catalog_oracle[..8], b"MTCAT001");
        let mut source_at = 8;
        for kind in 0..45 {
            let count = word(&catalog_oracle, source_at) as usize;
            assert_eq!(count, usize::from(info_bytes[kind * 116 + 0x4c]));
            assert_eq!(
                word(&catalog_oracle, source_at + 4),
                word(info_bytes, kind * 116 + 0x70)
            );
            source_at += 8;
            for slot in 0..count {
                assert_eq!(
                    &catalog_oracle[source_at..source_at + 8],
                    &product_bytes[kind * 48 + slot * 8..kind * 48 + slot * 8 + 8],
                );
                source_at += 8;
            }
        }
        assert_eq!(source_at, catalog_oracle.len());

        let list = crate::ai_ownership::BusinessList {
            entries: entries
                .chunks_exact(28)
                .map(|raw| raw.try_into().unwrap())
                .collect(),
            active_count: owned as u16,
        };
        assert_eq!(
            list.entries
                .iter()
                .map(|entry| word(entry, 4))
                .collect::<Vec<_>>(),
            vec![9, 10, 11, 12, 13, 14],
        );
        let buildings: Vec<[u8; 64]> = building_prefixes
            .chunks_exact(64)
            .map(|raw| raw.try_into().unwrap())
            .collect();
        let info = crate::business_pool::BusinessCatalog::from_rows(std::array::from_fn(|i| {
            info_bytes[i * 116..(i + 1) * 116].try_into().unwrap()
        }));
        let settings = crate::economy::BusinessDaySettings::from_catalog(&info);
        let mut catalogs = crate::shopping::Catalogs {
            commodities: std::array::from_fn(|_| vec![]),
            fractions: std::array::from_fn(|_| vec![]),
        };
        for kind in 0..45 {
            for slot in 0..usize::from(info_bytes[kind * 116 + 0x4c]) {
                catalogs.commodities[kind].push(word(product_bytes, kind * 48 + slot * 8));
                catalogs.fractions[kind].push(f32::from_bits(word(
                    product_bytes,
                    kind * 48 + slot * 8 + 4,
                )));
            }
        }
        let prices = std::array::from_fn(|i| f32::from_bits(word(commodity_bytes, i * 28 + 0x14)));
        let inputs = BusinessDayInputs {
            buildings: &buildings,
            building_categories: &[],
            category_owners: &[0; 52],
            catalogs: &catalogs,
            unit_prices: &prices,
            settings: &settings,
        };
        let mut businesses: Vec<[u8; 544]> = before
            .chunks_exact(544)
            .map(|raw| raw.try_into().unwrap())
            .collect();
        let mut commands = crate::economy::LocalBusinessCommands::default();
        update(
            &list,
            &mut businesses,
            &inputs,
            0,
            0,
            &mut commands,
            |_, _| panic!("saved retail settings applied before queue deadline"),
        )
        .unwrap();
        let differences: Vec<_> = businesses
            .as_flattened()
            .iter()
            .zip(expected)
            .enumerate()
            .filter(|(_, (actual, expected))| actual != expected)
            .map(|(at, (actual, expected))| (at / 544, at % 544, *actual, *expected))
            .take(20)
            .collect();
        assert!(
            differences.is_empty(),
            "saved AI retail rows: {differences:?}"
        );
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
        assert_eq!(
            payloads, expected_payloads,
            "six original saved retail terms"
        );
    }
}
