//! Original0047f930/0047f970 lazy city price and residential-rent cache.

pub struct Inputs<'a> {
    pub businesses: &'a [[u8; 544]],
    pub buildings: &'a [[u8; 64]],
    pub catalogs: &'a crate::shopping::Catalogs,
    pub commodities: &'a [crate::resident::Commodity; 32],
}

/// Only the price/rent cache portion of the larger original statistics object.
/// World reset/load callers invalidate it; occupancy alone does not refresh it.
pub struct Cache {
    pub dirty: bool,
    pub prices: [f32; 32],
    pub rent: f32,
}

impl Cache {
    /// Mark the lazy refresh pending without pretending to execute the rest of
    /// the original statistics reset0047f8e0 and its unrelated callees.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub fn price(&mut self, commodity: usize, inputs: &Inputs<'_>) -> Result<f32, String> {
        self.ensure_current(inputs)?;
        self.prices
            .get(commodity)
            .copied()
            .ok_or("market commodity outside table".into())
    }

    pub fn residential_rent(&mut self, inputs: &Inputs<'_>) -> Result<f32, String> {
        self.ensure_current(inputs)?;
        Ok(self.rent)
    }

    fn ensure_current(&mut self, inputs: &Inputs<'_>) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        // Both native getters clear this before calling0047faa0. Failed
        // validation preserves mutations already made and does not retry.
        self.dirty = false;
        if inputs.businesses.len() != 600 || inputs.buildings.len() != 600 {
            return Err("market refresh requires complete business/building pools".into());
        }
        self.prices.fill(0.);
        for (commodity, cached) in self.prices.iter_mut().enumerate() {
            let mut count = 0_i32;
            for business in inputs.businesses {
                if business[8..10] == [255, 255] {
                    continue;
                }
                // Eligibility462ec0 uses catalog membership, not the business
                // active flag, discriminator, stock or opening hours.
                let kind = u32::from_le_bytes(business[4..8].try_into().unwrap()) as usize;
                let catalog = inputs
                    .catalogs
                    .commodities
                    .get(kind)
                    .ok_or("market business kind outside catalog table")?;
                let Some(slot) = catalog.iter().position(|&id| id == commodity as u32) else {
                    continue;
                };
                if slot >= 6 {
                    return Err("market catalog exceeds native price slots".into());
                }
                let at = 0x1c4 + slot * 4;
                let price = f32::from_le_bytes(business[at..at + 4].try_into().unwrap());
                *cached = (f64::from(*cached) + f64::from(price)) as f32;
                count += 1;
            }
            let baseline = inputs.commodities[commodity];
            *cached = ((f64::from(baseline.value) * f64::from(baseline.markup)
                + f64::from(*cached))
                / f64::from(count + 1)) as f32;
        }
        self.rent = 0.;
        let mut capacity = 0_i32;
        for building in inputs.buildings {
            if building[0x3f] == 0 {
                continue;
            }
            let index = u32::from_le_bytes(building[0x1c..0x20].try_into().unwrap());
            if index == u32::MAX {
                continue;
            }
            let business = inputs
                .businesses
                .get(index as usize)
                .ok_or("market building business outside pool")?;
            if u32::from_le_bytes(business[..4].try_into().unwrap()) != 1 {
                continue;
            }
            let units = u16::from_le_bytes(business[0xe..0x10].try_into().unwrap());
            let rent = f32::from_le_bytes(business[0x1dc..0x1e0].try_into().unwrap());
            self.rent = (f64::from(rent) * f64::from(units) + f64::from(self.rent)) as f32;
            capacity = capacity.wrapping_add(i32::from(units));
        }
        // Zero capacity is unguarded in the original x87 calculation.
        self.rent = (f64::from(self.rent) / f64::from(capacity)) as f32;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original full market/rent cache observations"]
    fn matches_native_cache() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_MARKET_CACHE")).unwrap();
        assert_eq!(&bytes[..8], b"MTCACHE1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        let mut at = 12;
        for case in 0..word(8) {
            let getter = word(at);
            let commodity = word(at + 4) as usize;
            at += 8;
            let mut cache = Cache {
                dirty: word(at) != 0,
                prices: std::array::from_fn(|i| float(at + 4 + i * 4)),
                rent: float(at + 132),
            };
            at += 136;
            let businesses: Vec<[u8; 544]> = bytes[at..at + 600 * 544]
                .chunks_exact(544)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 600 * 544;
            let buildings: Vec<[u8; 64]> = bytes[at..at + 600 * 64]
                .chunks_exact(64)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 600 * 64;
            let mut catalogs = crate::shopping::Catalogs {
                commodities: std::array::from_fn(|_| Vec::new()),
                fractions: std::array::from_fn(|_| Vec::new()),
            };
            for catalog in &mut catalogs.commodities {
                let count = word(at) as usize;
                at += 4;
                *catalog = (0..count).map(|i| word(at + i * 4)).collect();
                at += count * 4;
            }
            let commodities = std::array::from_fn(|i| crate::resident::Commodity {
                active: false,
                value: float(at + i * 8),
                markup: float(at + i * 8 + 4),
            });
            at += 32 * 8;
            let inputs = Inputs {
                businesses: &businesses,
                buildings: &buildings,
                catalogs: &catalogs,
                commodities: &commodities,
            };
            let result = match getter {
                0 => cache.residential_rent(&inputs),
                1 => cache.price(commodity, &inputs),
                _ => panic!("unsupported fixture getter"),
            }
            .unwrap();
            assert_eq!(u32::from(cache.dirty), word(at), "case {case} dirty");
            for (i, price) in cache.prices.iter().enumerate() {
                assert_eq!(
                    price.to_bits(),
                    word(at + 4 + i * 4),
                    "case {case} commodity {i}"
                );
            }
            assert_eq!(cache.rent.to_bits(), word(at + 132), "case {case} rent");
            at += 136;
            assert_eq!(result.to_bits(), word(at), "case {case} getter");
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }
}
