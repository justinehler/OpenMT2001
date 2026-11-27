//! Original save container from 004bbe30 / 004bc290.
//! Unknown payloads and fields remain opaque.
use crate::pe::PeImage;

pub mod appearance;
pub mod demolition;
pub mod extension;
pub mod restore;
pub mod writer;

#[derive(Debug)]
pub struct Section<'a> {
    pub tag: u32,
    pub offset: usize,
    pub payload: &'a [u8],
}

#[derive(Debug)]
pub struct SaveFile<'a> {
    pub sections: Vec<Section<'a>>,
    /// The writer reserves eight bytes for the terminator. Its second word
    /// is not interpreted by the reader; preserve it rather than guessing.
    pub terminator_padding: [u8; 4],
}

impl<'a> SaveFile<'a> {
    /// Original 4bc4a5 copies this metadata before level selection. The
    /// independent year/cash words are retained even though selection only
    /// consumes the first and fourth words.
    pub fn level_metadata(&self) -> Result<[u32; 4], String> {
        let bytes = self.table(0x2000, 16)?;
        Ok(std::array::from_fn(|i| {
            u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
        }))
    }

    /// Original 483dd0/483df0 consume two distinct camera allocations.
    /// Headless callers retain the exact records for their presentation owner.
    pub fn cameras(&self) -> Result<[&'a [u8; 196]; 2], String> {
        Ok([
            self.table(0x100, 196)?.try_into().unwrap(),
            self.table(0x100000, 196)?.try_into().unwrap(),
        ])
    }

    /// 4810e0 visits repeated 4000 records in stream order. A kind-zero
    /// record owns a packet whose size word is at packet+8.
    pub fn scheduled(&self) -> Result<Vec<SavedScheduled<'a>>, String> {
        self.sections
            .iter()
            .filter(|s| s.tag == 0x4000)
            .map(|section| {
                let bytes = section.payload;
                let header: &[u8; 24] = bytes
                    .get(..24)
                    .ok_or("truncated scheduled header")?
                    .try_into()
                    .unwrap();
                let kind = u32::from_le_bytes(header[..4].try_into().unwrap());
                let packet = if kind == 0 {
                    let packet = bytes.get(24..).ok_or("scheduled packet missing")?;
                    if packet.len() < 16
                        || u32::from_le_bytes(packet[8..12].try_into().unwrap()) as usize
                            != packet.len()
                    {
                        return Err(
                            "scheduled packet length disagrees with its native size word".into(),
                        );
                    }
                    Some(packet)
                } else {
                    if bytes.len() != 24 {
                        return Err("non-packet scheduled record has trailing bytes".into());
                    }
                    None
                };
                Ok(SavedScheduled { header, packet })
            })
            .collect()
    }

    /// 465810 copies the 0x158-byte auction controller at 7f4758.
    pub fn auction(&self) -> Result<Option<&'a [u8; 344]>, String> {
        let mut records = self.sections.iter().filter(|s| s.tag == 0x8000);
        let Some(record) = records.next() else {
            return Ok(None);
        };
        if records.next().is_some() {
            return Err("multiple saved auction allocations".into());
        }
        Ok(Some(
            record
                .payload
                .try_into()
                .map_err(|_| "saved auction must contain 344 bytes")?,
        ))
    }

    pub fn game_mode(&self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.table(0x400000, 4)?.try_into().unwrap(),
        ))
    }

    pub fn scenario_objective(&self) -> Result<crate::scenario_objective::State, String> {
        crate::scenario_objective::State::from_bytes(self.table(0x10000, 60)?)
    }
    /// Original4bb730/4bb750 preserve the complete player-history block.
    pub fn player_history(&self) -> Result<crate::player_history::State, String> {
        crate::player_history::State::from_bytes(
            self.table(0x20000, crate::player_history::RECORD_BYTES)?,
        )
    }
    /// Original4b0f30/4b0f50 persist the complete construction/demolition pool.
    pub fn pending_world(&self) -> Result<crate::pending_world::State, String> {
        crate::pending_world::State::from_bytes(self.table(0x800, 0xe04)?)
    }

    /// 0045be80 rebuilds the 0x150-stride node pool from repeated tag10
    /// payloads. 0045b3b0 restores the separate 2000-row account array.
    pub fn residents(&self) -> Result<SavedResidents<'a>, String> {
        let mut nodes = Vec::new();
        for section in self.sections.iter().filter(|s| s.tag == 0x10) {
            nodes.push(
                section
                    .payload
                    .try_into()
                    .map_err(|_| "resident node must contain 0x14a saved bytes")?,
            );
        }
        if nodes.len() > 2000 {
            return Err("resident node count exceeds original pool".into());
        }
        Ok(SavedResidents {
            nodes,
            accounts: self.table(0x1000, 2000 * 0x90)?,
        })
    }

    /// 004bc290 copies tag 0x2000 to 00920278. The load path passes its
    /// fourth word (00920284) to difficulty selector 0048d130.
    pub fn difficulty(&self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.table(0x2000, 16)?[12..16].try_into().unwrap(),
        ))
    }

    /// Raw one-based owner byte from a saved category/block prefix.
    /// 00447d70 copies these prefixes to 00617628 with runtime stride 0x17c.
    pub fn category_owner(&self, category: usize) -> Result<u8, String> {
        if category >= 52 {
            return Err("category exceeds the native saved block table".into());
        }
        Ok(self.table(4, 0xb270)?[0x9600 + category * 0x8c + 5])
    }

    /// Serialized dwords+7c/+80 consumed by the hourly category pass00447e20.
    pub fn category_countdowns(&self, category: usize) -> Result<[u32; 2], String> {
        if category >= 52 {
            return Err("category exceeds the native saved block table".into());
        }
        let at = 0x9600 + category * 0x8c + 0x7c;
        let table = self.table(4, 0xb270)?;
        Ok(std::array::from_fn(|i| {
            u32::from_le_bytes(table[at + i * 4..at + i * 4 + 4].try_into().unwrap())
        }))
    }

    /// Serialized category+78, set to2 by ownership expiry0048c450.
    pub fn category_ownership_state(&self, category: usize) -> Result<u32, String> {
        if category >= 52 {
            return Err("category exceeds the native saved block table".into());
        }
        let at = 0x9600 + category * 0x8c + 0x78;
        Ok(u32::from_le_bytes(
            self.table(4, 0xb270)?[at..at + 4].try_into().unwrap(),
        ))
    }

    /// Original category+70 commodity membership mask retained by the save.
    pub fn category_commodity_mask(&self, category: usize) -> Result<u32, String> {
        if category >= 52 {
            return Err("category exceeds native block table".into());
        }
        let at = 0x9600 + category * 0x8c + 0x70;
        Ok(u32::from_le_bytes(
            self.table(4, 0xb270)?[at..at + 4].try_into().unwrap(),
        ))
    }

    /// Runtime category +88, incremented by resident placement0045c3d0.
    pub fn category_resident_counter(&self, category: usize) -> Result<u32, String> {
        if category >= 52 {
            return Err("category exceeds native block table".into());
        }
        let at = 0x9600 + category * 0x8c + 0x88;
        Ok(u32::from_le_bytes(
            self.table(4, 0xb270)?[at..at + 4].try_into().unwrap(),
        ))
    }

    /// Runtime category +84 (6176ac), copied unchanged by 00447d70 and
    /// consumed by the housing category score 0045a4b0.
    pub fn category_prestige(&self, category: usize) -> Result<f32, String> {
        if category >= 52 {
            return Err("category exceeds native block table".into());
        }
        let at = 0x9600 + category * 0x8c + 0x84;
        Ok(f32::from_le_bytes(
            self.table(4, 0xb270)?[at..at + 4].try_into().unwrap(),
        ))
    }

    /// Comparator bytes used by 00462070, in telecom/gas/water/electrical order.
    /// Enable flags are separate runtime state and are not inferred here.
    pub fn utility_owners(&self) -> Result<[u8; 4], String> {
        Ok([
            self.category_owner(2)?,
            self.category_owner(35)?,
            self.category_owner(39)?,
            self.category_owner(42)?,
        ])
    }

    /// Category table 00541ebc consumed by the separate 0048df00 payout.
    pub fn income_property_owners(&self) -> Result<[u8; 4], String> {
        Ok([
            self.category_owner(4)?,
            self.category_owner(17)?,
            self.category_owner(43)?,
            self.category_owner(44)?,
        ])
    }

    /// Persisted clock words from 0047f450 and 004bb0d0. Load without
    /// recomputing the linear index or invoking player/UI load side effects.
    pub fn clock(&self) -> Result<SavedClock, String> {
        let calendar = self.table(0x200, 20)?;
        let timing = self.table(0x400, 24)?;
        Ok(SavedClock {
            calendar: std::array::from_fn(|i| {
                u32::from_le_bytes(calendar[i * 4..i * 4 + 4].try_into().unwrap())
            }),
            timing: std::array::from_fn(|i| {
                u32::from_le_bytes(timing[i * 4..i * 4 + 4].try_into().unwrap())
            }),
        })
    }

    /// 0048c770 writes ordered metadata/player pairs. 0048c800 rebuilds
    /// pointers in this order; business owner numbers are one-based.
    pub fn players(&self) -> Result<Vec<SavedPlayer<'a>>, String> {
        let mut sections = self.sections.iter();
        let mut players = Vec::new();
        while let Some(section) = sections.next() {
            if section.tag == 0x40 {
                return Err("saved player has no preceding metadata".into());
            }
            if section.tag != 0x80000 {
                continue;
            }
            let payload = sections
                .next()
                .ok_or("player metadata has no player payload")?;
            if section.payload.len() != 0x30
                || payload.tag != 0x40
                || payload.payload.len() != 0x1b8
            {
                return Err("invalid saved player pair".into());
            }
            players.push(SavedPlayer {
                metadata: section.payload.try_into().unwrap(),
                bytes: payload.payload.try_into().unwrap(),
            });
        }
        Ok(players)
    }

    /// Fixed tables copied by 00463c80, 00463cc0, and 00464460.
    /// This exposes saved state; it does not perform the game's load fixups.
    pub fn economy(&self) -> Result<SavedEconomy<'a>, String> {
        Ok(SavedEconomy {
            businesses: self.table(0x20, 600 * 0x220)?,
            business_info: self.table(0x40000, 45 * 0x74)?,
            commodities: self.table(0x200000, 32 * 0x1c)?,
        })
    }

    /// Return exact fixed table prefixes for the retained city; script-built
    /// catalog ordering remains separately owned by the replayed Lua context.
    pub fn business_catalog(&self) -> Result<[[u8; 0x74]; 45], String> {
        let bytes = self.table(0x40000, 45 * 0x74)?;
        Ok(std::array::from_fn(|i| {
            bytes[i * 0x74..(i + 1) * 0x74].try_into().unwrap()
        }))
    }

    pub fn commodity_rows(&self) -> Result<[[u8; 0x1c]; 32], String> {
        let bytes = self.table(0x200000, 32 * 0x1c)?;
        Ok(std::array::from_fn(|i| {
            bytes[i * 0x1c..(i + 1) * 0x1c].try_into().unwrap()
        }))
    }

    pub fn building_record(&self) -> Result<&'a [u8; 0xb270], String> {
        Ok(self.table(4, 0xb270)?.try_into().unwrap())
    }

    fn table(&self, tag: u32, length: usize) -> Result<&'a [u8], String> {
        let mut matching = self.sections.iter().filter(|s| s.tag == tag);
        let section = matching
            .next()
            .ok_or_else(|| format!("missing save table {tag:x}"))?;
        if matching.next().is_some() || section.payload.len() != length {
            return Err(format!(
                "ambiguous or incorrect length for save table {tag:x}"
            ));
        }
        Ok(section.payload)
    }

    /// Rebuild category membership from IDs, as 00447d70 does, without pointers.
    pub fn buildings(&self) -> Result<Vec<SavedBuilding<'a>>, String> {
        let payload = self.table(4, 0xb270)?;
        let mut result = Vec::new();
        for (category, record) in payload[0x9600..].chunks_exact(0x8c).enumerate() {
            let count = i32::from_le_bytes(record[8..12].try_into().unwrap());
            if !(0..=32).contains(&count) {
                return Err("saved building category count exceeds its ID array".into());
            }
            for id in record[12..12 + count as usize * 4].chunks_exact(4) {
                let id = i32::from_le_bytes(id.try_into().unwrap());
                if id == -1 {
                    continue;
                }
                if !(0..600).contains(&id) {
                    return Err("saved category references an invalid building ID".into());
                }
                let at = id as usize * 0x40;
                result.push(SavedBuilding {
                    category,
                    slot: id as usize,
                    bytes: payload[at..at + 0x40].try_into().unwrap(),
                });
            }
        }
        Ok(result)
    }

    pub(crate) fn building_prefixes(&self) -> Result<&'a [u8], String> {
        Ok(&self.table(4, 0xb270)?[..0x9600])
    }

    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let mut at = 0usize;
        let mut sections = Vec::new();
        loop {
            let header_end = at.checked_add(8).ok_or("save offset overflow")?;
            let header = bytes
                .get(at..header_end)
                .ok_or("truncated save section header or missing terminator")?;
            let tag = u32::from_le_bytes(header[..4].try_into().unwrap());
            if tag == u32::MAX {
                if header_end != bytes.len() {
                    return Err("trailing data after save terminator".into());
                }
                return Ok(Self {
                    sections,
                    terminator_padding: header[4..8].try_into().unwrap(),
                });
            }
            let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
            let end = header_end
                .checked_add(length)
                .ok_or("save section length overflow")?;
            let payload = bytes
                .get(header_end..end)
                .ok_or("truncated save section payload")?;
            sections.push(Section {
                tag,
                offset: at,
                payload,
            });
            at = end;
        }
    }

    /// This checks the original compatibility record, not payload semantics.
    pub fn validate_compatibility(&self, executable: &PeImage<'_>) -> Result<(), String> {
        let header = self
            .sections
            .first()
            .ok_or("save has no compatibility record")?;
        if header.tag != 1 || header.payload != executable.at(0x545d1c, 40)? {
            return Err("save compatibility record differs from the supplied executable".into());
        }
        Ok(())
    }
}

/// Original bytes beginning at runtime player+4; pointer-shaped fields remain opaque.
#[derive(Debug)]
pub struct SavedPlayer<'a> {
    pub metadata: &'a [u8; 0x30],
    pub bytes: &'a [u8; 0x1b8],
}

#[derive(Debug)]
pub struct SavedScheduled<'a> {
    pub header: &'a [u8; 24],
    pub packet: Option<&'a [u8]>,
}

/// Original clock state. Calendar order: hour, year, era, absolute hour, day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedClock {
    /// Globals 00822950, 00822954, 00822958, 0082295c, 00822960.
    /// The fourth word is persisted independently, even when inconsistent.
    pub calendar: [u32; 5],
    /// Writer order: 00822d40, 00822d44, 00822948, 00822d48,
    /// 0086da44, 00822d30. These are not operating-system timestamps.
    pub timing: [u32; 6],
}

impl SavedPlayer<'_> {
    /// Cash at runtime +4, updated by 0048a5a0.
    pub fn cash(&self) -> f32 {
        f32::from_le_bytes(self.bytes[..4].try_into().unwrap())
    }
}

/// Compact prefix saved by 00447cc0. The omitted runtime tail is regenerated.
#[derive(Debug)]
pub struct SavedBuilding<'a> {
    pub category: usize,
    pub slot: usize,
    pub bytes: &'a [u8; 0x40],
}

impl SavedBuilding<'_> {
    /// Inclusive lot endpoints, not width and depth.
    pub fn lot_endpoints(&self) -> [u8; 4] {
        self.bytes[4..8].try_into().unwrap()
    }

    pub fn height(&self) -> u8 {
        self.bytes[8]
    }

    pub fn center(&self) -> [f32; 2] {
        [
            f32::from_le_bytes(self.bytes[12..16].try_into().unwrap()),
            f32::from_le_bytes(self.bytes[16..20].try_into().unwrap()),
        ]
    }
}

#[derive(Debug)]
pub struct SavedEconomy<'a> {
    businesses: &'a [u8],
    business_info: &'a [u8],
    commodities: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct SavedBusiness<'a> {
    /// Includes unknown fields and serialized pointers, which remain opaque.
    pub bytes: &'a [u8; 0x220],
}

/// Serialized indices remain integers; runtime pointer fixups are not copied.
pub struct SavedResidents<'a> {
    pub nodes: Vec<&'a [u8; 0x14a]>,
    pub accounts: &'a [u8],
}

impl SavedBusiness<'_> {
    /// Index into SaveFile::players. Zero in the original record means no owner.
    pub fn owner_index(&self) -> Option<usize> {
        self.bytes[0x0a].checked_sub(1).map(usize::from)
    }
    /// 004722f0 expects this discriminator to be zero for its argument.
    pub fn discriminator(&self) -> u32 {
        u32::from_le_bytes(self.bytes[..4].try_into().unwrap())
    }

    pub fn business_type(&self) -> u32 {
        u32::from_le_bytes(self.bytes[4..8].try_into().unwrap())
    }

    pub fn building_index(&self) -> u16 {
        u16::from_le_bytes(self.bytes[8..10].try_into().unwrap())
    }

    /// Physical slot only; the BusinessInfo count determines which slots apply.
    pub fn stock_target(&self, slot: usize) -> Option<(u16, i16)> {
        if slot >= 6 {
            return None;
        }
        let stock = 0x80 + slot * 0x34;
        let target = 0x1b8 + slot * 2;
        Some((
            u16::from_le_bytes(self.bytes[stock..stock + 2].try_into().unwrap()),
            i16::from_le_bytes(self.bytes[target..target + 2].try_into().unwrap()),
        ))
    }
}

impl<'a> SavedEconomy<'a> {
    pub fn daily_settings(&self) -> crate::economy::BusinessDaySettings {
        let catalog =
            crate::business_pool::BusinessCatalog::from_rows(std::array::from_fn(|kind| {
                self.business_info[kind * 0x74..(kind + 1) * 0x74]
                    .try_into()
                    .unwrap()
            }));
        crate::economy::BusinessDaySettings::from_catalog(&catalog)
    }

    /// Resolve supplied saved-day inputs without executing scripts. Multi-product
    /// catalog order must be recovered before such a business can be updated.
    pub fn daily_business_parameters(
        &self,
        buildings: &[SavedBuilding<'_>],
        category_owners: &[u8; 52],
    ) -> Result<Vec<Option<crate::economy::BusinessDayParameters>>, String> {
        self.businesses()
            .map(|business| {
                if business.bytes[0x21c] == 0 {
                    return Ok(None);
                }
                let building = buildings
                    .iter()
                    .find(|b| b.slot == usize::from(business.building_index()))
                    .ok_or("active business references an unavailable building")?;
                let kind = business.business_type();
                let count = self.commodity_count(kind).ok_or("invalid business type")?;
                let mut prices = Vec::new();
                if business.discriminator() == 0 && count != 0 {
                    let [id] = self.single_product_catalog(kind)?;
                    prices.push(
                        self.commodity_value(id)
                            .ok_or("invalid commodity identity")?,
                    );
                }
                let [x0, y0, x1, y1] = building.lot_endpoints().map(i32::from);
                let at = kind as usize * 0x74 + 0x70;
                Ok(Some(crate::economy::BusinessDayParameters {
                    capacity: self.stock_capacity(&business, building)?,
                    catalog_mask: u32::from_le_bytes(
                        self.business_info[at..at + 4].try_into().unwrap(),
                    ),
                    prices,
                    utility_scalar: (x1 - x0 + 1) * (y1 - y0 + 1) * i32::from(building.height()),
                    utility_rates: self
                        .utility_rates(kind)
                        .ok_or("invalid business utility rates")?,
                    landlord: *category_owners
                        .get(building.category)
                        .ok_or("invalid landlord category")?,
                }))
            })
            .collect()
    }

    /// BusinessInfo +60/+64/+68/+6c: telecom, gas, water, electrical.
    /// These rates are multiplied after the footprint-height f32 store.
    pub fn utility_rates(&self, business_type: u32) -> Option<[f32; 4]> {
        if business_type >= 45 {
            return None;
        }
        Some(std::array::from_fn(|i| {
            let at = business_type as usize * 0x74 + 0x60 + i * 4;
            f32::from_le_bytes(self.business_info[at..at + 4].try_into().unwrap())
        }))
    }

    /// A one-product saved count/mask has only one possible slot identity.
    /// Verified against original AddCommodity calls for all six supplied
    /// saved retail businesses. Multi-product order still requires scripts.
    pub fn single_product_catalog(&self, business_type: u32) -> Result<[u32; 1], String> {
        if self.commodity_count(business_type) != Some(1) {
            return Err("saved catalog is not a single product".into());
        }
        let at = business_type as usize * 0x74 + 0x70;
        let mask = u32::from_le_bytes(self.business_info[at..at + 4].try_into().unwrap());
        if mask.count_ones() != 1 {
            return Err("single-product saved mask is inconsistent".into());
        }
        Ok([mask.trailing_zeros()])
    }

    /// Catalog slot order comes from script initialization, not the save.
    pub fn restock_lines(
        &self,
        business: &SavedBusiness<'_>,
        catalog: &[u32],
    ) -> Result<Vec<crate::economy::RestockLine>, String> {
        let count = self
            .commodity_count(business.business_type())
            .ok_or("invalid saved business type")? as usize;
        if count > 6 || catalog.len() != count {
            return Err("catalog does not match saved commodity count".into());
        }
        catalog
            .iter()
            .enumerate()
            .map(|(slot, &id)| {
                let (stock, target) = business.stock_target(slot).unwrap();
                Ok(crate::economy::RestockLine {
                    stock,
                    target,
                    unit_cost: self
                        .commodity_value(id)
                        .ok_or("invalid catalog commodity ID")?,
                })
            })
            .collect()
    }

    /// 00461fb0 footprint and 00461c67 capacity calculation from loaded fields.
    pub fn stock_capacity(
        &self,
        business: &SavedBusiness<'_>,
        building: &SavedBuilding<'_>,
    ) -> Result<i32, String> {
        let kind = business.business_type();
        if kind >= 45 || usize::from(business.building_index()) != building.slot {
            return Err("business type or linked building is invalid".into());
        }
        let [x0, y0, x1, y1] = building.lot_endpoints().map(i32::from);
        let area = (y1 - y0 + 1) * (x1 - x0 + 1);
        let at = kind as usize * 0x74 + 0x58;
        let rate = f32::from_le_bytes(self.business_info[at..at + 4].try_into().unwrap());
        Ok(crate::economy::stock_capacity(
            area,
            building.height(),
            rate,
        ))
    }

    pub fn businesses(&self) -> impl ExactSizeIterator<Item = SavedBusiness<'a>> + '_ {
        self.businesses
            .chunks_exact(0x220)
            .map(|bytes| SavedBusiness {
                bytes: bytes.try_into().unwrap(),
            })
    }

    /// BusinessInfo+50 is the serialized delay consumed by0045f660.
    pub fn interaction_delays(&self) -> [f32; 45] {
        std::array::from_fn(|kind| {
            let at = kind * 0x74 + 0x50;
            f32::from_le_bytes(self.business_info[at..at + 4].try_into().unwrap())
        })
    }

    pub fn commodity_mask(&self, business_type: u32) -> Option<u32> {
        if business_type >= 45 {
            return None;
        }
        let at = business_type as usize * 0x74 + 0x70;
        Some(u32::from_le_bytes(
            self.business_info[at..at + 4].try_into().unwrap(),
        ))
    }

    pub fn commodity_count(&self, business_type: u32) -> Option<u8> {
        (business_type < 45).then(|| self.business_info[business_type as usize * 0x74 + 0x4c])
    }

    pub fn commodity_value(&self, commodity_id: u32) -> Option<f32> {
        if commodity_id >= 32 {
            return None;
        }
        let at = commodity_id as usize * 0x1c + 0x14;
        Some(f32::from_le_bytes(
            self.commodities[at..at + 4].try_into().unwrap(),
        ))
    }

    /// Resident allocation consumes the same saved commodity table, including
    /// active byte +10 and markup +18 in addition to retail value +14.
    pub fn resident_commodities(&self) -> [crate::resident::Commodity; 32] {
        std::array::from_fn(|id| {
            let at = id * 0x1c;
            let float = |offset| {
                f32::from_le_bytes(
                    self.commodities[at + offset..at + offset + 4]
                        .try_into()
                        .unwrap(),
                )
            };
            crate::resident::Commodity {
                active: self.commodities[at + 0x10] != 0,
                value: float(0x14),
                markup: float(0x18),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_pairs_preserve_order_and_reject_orphans() {
        let metadata = [0u8; 0x30];
        let mut one = [0u8; 0x1b8];
        let mut two = one;
        one[..4].copy_from_slice(&11f32.to_le_bytes());
        two[..4].copy_from_slice(&22f32.to_le_bytes());
        let mut save = SaveFile {
            sections: Vec::new(),
            terminator_padding: [0; 4],
        };
        for bytes in [&one, &two] {
            save.sections.push(Section {
                tag: 0x80000,
                offset: 0,
                payload: &metadata,
            });
            save.sections.push(Section {
                tag: 0x40,
                offset: 0,
                payload: bytes,
            });
        }
        assert_eq!(
            save.players()
                .unwrap()
                .iter()
                .map(SavedPlayer::cash)
                .collect::<Vec<_>>(),
            [11., 22.]
        );
        save.sections.remove(0);
        assert!(save.players().is_err());
        save.sections.clear();
        save.sections.push(Section {
            tag: 0x80000,
            offset: 0,
            payload: &metadata,
        });
        assert!(save.players().is_err());
    }
    #[test]
    fn validates_saved_category_membership() {
        let mut payload = vec![0u8; 0xb270];
        let parse = |payload: &[u8]| {
            let save = SaveFile {
                sections: vec![Section {
                    tag: 4,
                    offset: 0,
                    payload,
                }],
                terminator_padding: [0; 4],
            };
            save.buildings().map(|b| b.len())
        };
        assert_eq!(parse(&payload).unwrap(), 0);
        payload[0x9608..0x960c].copy_from_slice(&33u32.to_le_bytes());
        assert!(parse(&payload).is_err());
        payload[0x9608..0x960c].copy_from_slice(&1u32.to_le_bytes());
        payload[0x960c..0x9610].copy_from_slice(&600u32.to_le_bytes());
        assert!(parse(&payload).is_err());
        payload[0x960c..0x9610].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse(&payload).unwrap(), 0);
    }

    #[test]
    fn business_fields_keep_type_building_and_signed_target_distinct() {
        let mut bytes = [0u8; 0x220];
        bytes[4..8].copy_from_slice(&12u32.to_le_bytes());
        bytes[8..10].copy_from_slice(&9u16.to_le_bytes());
        bytes[0x80..0x82].copy_from_slice(&65535u16.to_le_bytes());
        bytes[0x1b8..0x1ba].copy_from_slice(&(-2i16).to_le_bytes());
        let business = SavedBusiness { bytes: &bytes };
        assert_eq!(business.business_type(), 12);
        assert_eq!(business.building_index(), 9);
        assert_eq!(business.stock_target(0), Some((65535, -2)));
        assert_eq!(business.stock_target(usize::MAX), None);
    }

    #[test]
    fn preserves_repeated_sections_and_opaque_terminator_word() {
        let bytes = [16u32, 0, 16, 0, u32::MAX, 0x12345678]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        let save = SaveFile::parse(&bytes).unwrap();
        assert_eq!(
            save.sections
                .iter()
                .map(|s| (s.tag, s.offset))
                .collect::<Vec<_>>(),
            [(16, 0), (16, 8)]
        );
        assert_eq!(save.terminator_padding, 0x12345678u32.to_le_bytes());
    }

    #[test]
    fn rejects_truncated_lengths_missing_terminator_and_trailing_bytes() {
        assert!(SaveFile::parse(&[]).is_err());
        assert!(SaveFile::parse(&[0xff; 4]).is_err());
        let section = [1u32, u32::MAX]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(SaveFile::parse(&section).is_err());
        let missing = [1u32, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(SaveFile::parse(&missing).is_err());
        let trailing = [u32::MAX, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(SaveFile::parse(&trailing).is_err());
    }

    #[test]
    #[ignore = "requires both supplied original saves and v1.4 executable"]
    fn reads_both_supplied_saves() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        for (path, size, sections, records) in [
            ("OPENMT2001_SAVE_ONE", 855336, 357, 340),
            ("OPENMT2001_SAVE_TWO", 909416, 517, 500),
        ] {
            let bytes = std::fs::read(std::env::var(path).unwrap()).unwrap();
            assert_eq!(bytes.len(), size);
            let save = SaveFile::parse(&bytes).unwrap();
            save.validate_compatibility(&image).unwrap();
            assert_eq!(save.utility_owners().unwrap(), [0; 4]);
            assert_eq!(save.income_property_owners().unwrap(), [0; 4]);
            let pending = save.pending_world().unwrap();
            assert_eq!(pending.to_bytes(), save.table(0x800, 0xe04).unwrap());
            assert!(pending.count <= crate::pending_world::CAPACITY as u32);
            assert_eq!(
                save.player_history().unwrap().as_bytes(),
                save.table(0x20000, crate::player_history::RECORD_BYTES)
                    .unwrap()
            );
            assert_eq!(
                save.scenario_objective().unwrap().to_bytes(),
                save.table(0x10000, 60).unwrap()
            );
            let difficulty = save.difficulty().unwrap();
            assert_eq!(difficulty, u32::from(path == "OPENMT2001_SAVE_ONE"));
            let settings = crate::economy::OwnerIncomeSettings::original_hub(difficulty).unwrap();
            assert_eq!(settings.multiplier, 1.);
            assert_eq!(settings.daily_freight, 800.);
            let clock = save.clock().unwrap();
            let (calendar, timing) = if path == "OPENMT2001_SAVE_ONE" {
                ([14, 0, 0, 14, 0], [3323, 3323, 198, 8948, 0, 1])
            } else {
                ([9, 0, 0, 18, 0], [98, 98, 98, 5723, 0, 1])
            };
            assert_eq!(clock.calendar, calendar);
            assert_eq!(clock.timing, timing);
            let players = save.players().unwrap();
            assert_eq!(players.len(), 1);
            let mut account = crate::economy::PlayerAccount::from_saved(&players[0]).unwrap();
            if path == "OPENMT2001_SAVE_ONE" {
                // Native 00461c40 / 0048a5a0, saved cafe, charged mode 0.
                assert_eq!(account.ledger(0x11).unwrap().to_bits(), 0xc364cccd);
                let economy = save.economy().unwrap();
                let business = economy.businesses().nth(9).unwrap();
                let buildings = save.buildings().unwrap();
                let building = buildings.iter().find(|b| b.slot == 9).unwrap();
                // Original default catalog: CAFE -> SNACK (8).
                let catalog = economy
                    .single_product_catalog(business.business_type())
                    .unwrap();
                assert_eq!(catalog, [8]);
                let mut lines = economy.restock_lines(&business, &catalog).unwrap();
                let capacity = economy.stock_capacity(&business, building).unwrap();
                assert_eq!(capacity, 18);
                let mut balances = crate::economy::BusinessAccount::from_saved(&business);
                let result = balances
                    .replenish(&mut lines, capacity, false, Some(&mut account))
                    .unwrap();
                assert_eq!(lines[0].stock, 18);
                assert_eq!(result.slots[0].turnover, 1);
                assert_eq!(balances.cost_balance.to_bits(), 0xc2af0000);
                assert_eq!(balances.balance.to_bits(), 0x42cb0000);
                assert_eq!(account.cash.to_bits(), 0x4620b768);
                assert_eq!(account.ledger(0x11).unwrap().to_bits(), 0xc387a666);
                assert_eq!(account.ledger(0x13).unwrap().to_bits(), 0xc0200000);
                assert_eq!(account.cash.to_bits(), 0x4620b768);
            }
            assert_eq!(
                players[0].cash().to_bits(),
                if path == "OPENMT2001_SAVE_ONE" {
                    0x46216168
                } else {
                    0x469c4000
                }
            );
            let mut buildings = save.buildings().unwrap();
            buildings.sort_by_key(|b| b.slot);
            if path == "OPENMT2001_SAVE_ONE" {
                assert_eq!(
                    buildings.iter().map(|b| b.slot).collect::<Vec<_>>(),
                    (0..15).collect::<Vec<_>>()
                );
                assert_eq!(buildings[0].lot_endpoints(), [0, 0, 2, 2]);
                assert_eq!(buildings[0].height(), 8);
                assert_eq!(buildings[0].center(), [-234., 247.5]);
                assert_eq!(buildings[14].lot_endpoints(), [4, 0, 4, 1]);
                assert_eq!(buildings[14].height(), 1);
                assert_eq!(buildings[14].center(), [117., -399.5]);
            } else {
                assert!(buildings.is_empty());
            }
            let economy = save.economy().unwrap();
            if path == "OPENMT2001_SAVE_ONE" {
                let observed = [
                    [3., 18., 12., 6.],
                    [2., 2., 4., 4.],
                    [3., 18., 12., 6.],
                    [6., 12., 12., 6.],
                    [24., 36., 24., 6.],
                    [1., 1., 1., 1.],
                ];
                let mut utilities = crate::economy::Utilities::new(save.utility_owners().unwrap());
                for (business, expected) in economy.businesses().skip(9).take(6).zip(observed) {
                    let building = &buildings[usize::from(business.building_index())];
                    let [x0, y0, x1, y1] = building.lot_endpoints().map(i32::from);
                    let scalar = (x1 - x0 + 1) * (y1 - y0 + 1) * i32::from(building.height());
                    let rates = economy.utility_rates(business.business_type()).unwrap();
                    let amounts = utilities.assess(scalar, rates, business.bytes[0x0a]);
                    assert_eq!(amounts, expected);
                    // Snapshot evidence, not proof of the historical gate state.
                    for (i, at) in [0x44, 0x38, 0x40, 0x3c].into_iter().enumerate() {
                        let saved =
                            f32::from_le_bytes(business.bytes[at..at + 4].try_into().unwrap());
                        assert_eq!(saved.to_bits(), (-expected[i]).to_bits());
                    }
                }
            }
            for business in economy.businesses().filter(|b| b.discriminator() == 0) {
                assert_eq!(business.owner_index(), Some(0));
                assert!(players.get(business.owner_index().unwrap()).is_some());
            }
            assert_eq!(economy.businesses().len(), 600);
            let businesses = economy
                .businesses()
                .enumerate()
                .filter(|(_, b)| b.discriminator() == 0)
                .map(|(slot, b)| {
                    (
                        slot,
                        b.business_type(),
                        b.building_index(),
                        b.stock_target(0).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            if path == "OPENMT2001_SAVE_ONE" {
                assert_eq!(
                    businesses,
                    [
                        (9, 12, 9, (2, 18)),
                        (10, 3, 10, (0, 16)),
                        (11, 14, 11, (2, 18)),
                        (12, 5, 12, (18, 48)),
                        (13, 13, 13, (30, 30)),
                        (14, 10, 14, (0, 16)),
                    ]
                );
                for ((_, kind, _, _), commodity) in businesses.into_iter().zip([8, 4, 8, 2, 7, 10])
                {
                    assert_eq!(economy.commodity_count(kind), Some(1));
                    assert_eq!(economy.single_product_catalog(kind).unwrap(), [commodity]);
                }
            } else {
                assert!(businesses.is_empty());
            }
            assert_eq!(economy.commodity_value(13), Some(2.0));
            assert_eq!(economy.commodity_value(32), None);
            assert_eq!(economy.commodity_count(u32::MAX), None);
            assert_eq!(save.sections.len(), sections);
            assert_eq!(
                save.sections.iter().filter(|s| s.tag == 0x10).count(),
                records
            );
            assert_eq!(
                (
                    save.sections[1].tag,
                    save.sections[1].offset,
                    save.sections[1].payload.len()
                ),
                (0x2000, 0x30, 0x10)
            );
        }
    }
}
