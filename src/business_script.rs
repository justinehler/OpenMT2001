//! Live original BusinessInfo settings and ordered six-slot commodity catalogs.
use crate::{
    business_pool::BusinessCatalog,
    lua4::{Runtime, Value},
};

pub struct State {
    pub info: BusinessCatalog,
    /// Original 007f3ed0 entries: raw commodity identity and fraction bits.
    pub products: [[[u32; 2]; 6]; 45],
}

pub struct Bindings {
    first: u32,
    percentage_scale: f64,
}

impl Bindings {
    pub fn register(
        runtime: &mut Runtime,
        image: &crate::pe::PeImage<'_>,
        first: u32,
    ) -> Result<Self, String> {
        first
            .checked_add(90)
            .ok_or("business callback ID overflow")?;
        for (name, index) in crate::shopping::business_names(image)? {
            runtime.set_number(name, index as f64)?;
        }
        for (name, index) in crate::resident::commodity_names(image)? {
            runtime.set_number(name, f64::from(index))?;
        }
        for (name, value) in [("CLASS_LOW", 0.), ("CLASS_MEDIUM", 1.), ("CLASS_HIGH", 2.)] {
            runtime.set_number(name, value)?;
        }
        runtime.execute("BusinessInfo table", b"BusinessInfo={}", &mut |_, _| {
            Err("unexpected registration callback".into())
        })?;
        for index in 0..45 {
            runtime.register_number_properties("openmt_business", first + index * 2)?;
            runtime.register_indexed_number_first(
                "openmt_business",
                "AddCommodity",
                first + 90,
                index,
            )?;
            runtime.execute(
                "BusinessInfo entry",
                format!("BusinessInfo[{index}]=openmt_business").as_bytes(),
                &mut |_, _| Err("unexpected registration callback".into()),
            )?;
        }
        runtime.set_number("TRUE", 1.)?;
        runtime.set_number("true", 1.)?;
        runtime.execute(
            "business constants",
            b"FALSE=nil; false=nil; openmt_business=nil",
            &mut |_, _| Err("unexpected registration callback".into()),
        )?;
        Ok(Self {
            first,
            percentage_scale: f64::from_le_bytes(image.at(0x5267e0, 8)?.try_into().unwrap()),
        })
    }

    pub fn contains(&self, id: u32) -> bool {
        id.checked_sub(self.first).is_some_and(|id| id <= 90)
    }

    pub fn handle(
        &self,
        id: u32,
        args: &[Value],
        state: &mut State,
        commodities: &crate::commodity_script::State,
    ) -> Result<Option<f64>, String> {
        let id = id
            .checked_sub(self.first)
            .filter(|id| *id <= 90)
            .ok_or("unknown business callback")?;
        if id == 90 {
            let Some(Value::Number(receiver)) = args.first() else {
                return Err("business method requires bound index".into());
            };
            let index = match args.get(1) {
                Some(Value::Number(v)) => crate::economy::ftol_low_word(*v) as u32,
                _ => 0,
            };
            // Full catalogs return before dereferencing the commodity row.
            let receiver = crate::economy::ftol_low_word(*receiver) as usize;
            if state
                .info
                .row(receiver)
                .ok_or("business receiver leaves table")?[0x4c]
                == 6
            {
                return Ok(None);
            }
            let row = commodities
                .rows
                .get(index as usize)
                .ok_or("commodity index leaves table")?;
            let identity = u32::from_le_bytes(row[..4].try_into().unwrap());
            let fraction = match args.get(2) {
                Some(Value::Number(v)) => (*v * self.percentage_scale) as f32,
                _ => 1.,
            };
            state.add_commodity(receiver, index, identity, fraction)?;
            return Ok(None);
        }
        let Some(Value::Bytes(key)) = args.first() else {
            return Err("business property key must be a string".into());
        };
        // Original registration004d9481..004d9626, generic setters004d6860..
        //004d6924. Widths are registration types, not inferred from defaults.
        let (offset, kind) = match key.as_slice() {
            b"available" => (0x30, 0),
            b"density" => (0x34, 1),
            b"timeinstore" => (0x50, 1),
            b"stockcapacity" => (0x58, 1),
            b"telecomneed" => (0x60, 1),
            b"gasneed" => (0x64, 1),
            b"waterneed" => (0x68, 1),
            b"electricalneed" => (0x6c, 1),
            b"refurbcost" => (0x54, 2),
            b"minarea" => (0x38, 2),
            b"maxarea" => (0x3c, 2),
            b"maxside" => (0x40, 2),
            b"minheight" => (0x48, 2),
            b"maxheight" => (0x44, 2),
            b"openingtime" => (0x5c, 3),
            b"closingtime" => (0x5d, 3),
            b"footprintband" => (0x5e, 3),
            b"type" if id % 2 == 0 => return Ok(Some(f64::from(id / 2))),
            _ => return Err("unrecovered or read-only business property".into()),
        };
        let row = state.info.row_mut((id / 2) as usize).unwrap();
        if id % 2 == 1 {
            let Some(Value::Number(value)) = args.get(1) else {
                return Err("business setter requires numeric bridge".into());
            };
            match kind {
                0 => row[offset] = u8::from(*value != 0. && !value.is_nan()),
                1 => row[offset..offset + 4].copy_from_slice(&(*value as f32).to_le_bytes()),
                2 => row[offset..offset + 4]
                    .copy_from_slice(&(crate::economy::ftol_low_word(*value) as u32).to_le_bytes()),
                _ => row[offset] = crate::economy::ftol_low_word(*value) as u8,
            }
            Ok(None)
        } else {
            Ok(Some(match kind {
                0 => f64::from(row[offset]),
                1 => f64::from(f32::from_le_bytes(
                    row[offset..offset + 4].try_into().unwrap(),
                )),
                2 => f64::from(i32::from_le_bytes(
                    row[offset..offset + 4].try_into().unwrap(),
                )),
                _ => f64::from(row[offset] as i8),
            }))
        }
    }
}

impl State {
    /// Resolve the original ordered catalog against current commodity values
    /// for construction/restocking, without rebuilding it from script text.
    pub fn stock_products(
        &self,
        business: usize,
        commodities: &crate::commodity_script::State,
    ) -> Result<Vec<crate::building_binding::StockProduct>, String> {
        let row = self
            .info
            .row(business)
            .ok_or("business index leaves table")?;
        let catalog = self.products.get(business).unwrap();
        catalog
            .get(..usize::from(row[0x4c]))
            .ok_or("corrupt business commodity count")?
            .iter()
            .map(|entry| {
                let commodity = commodities
                    .rows
                    .get(entry[0] as usize)
                    .ok_or("catalog commodity identity leaves table")?;
                Ok(crate::building_binding::StockProduct {
                    value: f32::from_le_bytes(commodity[0x14..0x18].try_into().unwrap()),
                    fraction: f32::from_bits(entry[1]),
                })
            })
            .collect()
    }

    pub fn from_v14(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        Ok(Self {
            info: BusinessCatalog::from_pe(image)?,
            products: [[[0; 2]; 6]; 45],
        })
    }

    /// Original 004d8ff0 append/dedup tail. Commodity index and stored identity
    /// differ deliberately: the mask uses the former, deduplication the latter.
    pub fn add_commodity(
        &mut self,
        receiver: usize,
        index: u32,
        identity: u32,
        fraction: f32,
    ) -> Result<(), String> {
        let row = self
            .info
            .row_mut(receiver)
            .ok_or("business receiver leaves table")?;
        let count = usize::from(row[0x4c]);
        if count == 6 {
            return Ok(());
        }
        if count > 6 {
            return Err("corrupt business commodity count".into());
        }
        let kind = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
        let products = self
            .products
            .get_mut(kind)
            .ok_or("business identity leaves catalog")?;
        if products[..count].iter().any(|entry| entry[0] == identity) {
            return Ok(());
        }
        products[count] = [identity, fraction.to_bits()];
        row[0x4c] += 1;
        let mask =
            u32::from_le_bytes(row[0x70..0x74].try_into().unwrap()) | 1_u32.wrapping_shl(index);
        row[0x70..0x74].copy_from_slice(&mask.to_le_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original AddCommodity callback fixture"]
    fn matches_native_add_commodity_arguments_and_boundaries() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let data = std::fs::read(crate::fixtures::path("NATIVE_ADD_COMMODITY")).unwrap();
        assert_eq!(&data[..8], b"MTACOM01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let receiver = word(at) as usize;
            let raw_business = word(at + 4) as usize;
            let commodity = word(at + 8) as usize;
            let identity = word(at + 12);
            let count = word(at + 16);
            let mask = word(at + 20);
            let kind = data[at + 24];
            let percentage = f32::from_bits(word(at + 28));
            assert_eq!(word(at + 32), 0x100f0000);
            at += 36;
            let mut state = State::from_v14(&image).unwrap();
            let row = state.info.row_mut(receiver).unwrap();
            row[..4].copy_from_slice(&(raw_business as u32).to_le_bytes());
            row[0x4c] = count as u8;
            row[0x70..0x74].copy_from_slice(&mask.to_le_bytes());
            let initial_count = word(at) as usize;
            at += 4;
            for slot in 0..initial_count {
                state.products[raw_business][slot] = [word(at), word(at + 4)];
                at += 8;
            }
            let mut commodities = crate::commodity_script::State::from_v14(&image, [0; 5]).unwrap();
            commodities.rows[commodity][..4].copy_from_slice(&identity.to_le_bytes());
            let mut runtime = Runtime::new().unwrap();
            let bindings = Bindings::register(&mut runtime, &image, 100).unwrap();
            let optional = match kind {
                2 => format!(",{percentage}"),
                4 => format!(",'{percentage}'"),
                255 => String::new(),
                _ => ",nil".into(),
            };
            runtime
                .execute(
                    "native AddCommodity",
                    format!("BusinessInfo[{receiver}]:AddCommodity({commodity}{optional})")
                        .as_bytes(),
                    &mut |id, args| bindings.handle(id, args, &mut state, &commodities),
                )
                .unwrap();
            let expected_count = word(at) as usize;
            assert_eq!(
                usize::from(state.info.row(receiver).unwrap()[0x4c]),
                expected_count,
                "case {case}"
            );
            assert_eq!(
                u32::from_le_bytes(
                    state.info.row(receiver).unwrap()[0x70..0x74]
                        .try_into()
                        .unwrap()
                ),
                word(at + 4),
                "case {case}"
            );
            at += 8;
            for slot in 0..expected_count {
                assert_eq!(
                    state.products[raw_business][slot],
                    [word(at), word(at + 4)],
                    "case {case}, slot {slot}"
                );
                at += 8;
            }
            let calls = word(at) as usize;
            at += 4 + calls * 4;
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original v1.4 settings and native catalog fixture"]
    fn executes_complete_business_settings() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let source = std::fs::read(crate::fixtures::path("BUSINESS_SETTINGS")).unwrap();
        let native = std::fs::read(crate::fixtures::path("NATIVE_CATALOG")).unwrap();
        let mut state = State::from_v14(&image).unwrap();
        let mut commodities = crate::commodity_script::State::from_v14(&image, [0; 5]).unwrap();
        let mut runtime = Runtime::new().unwrap();
        let bindings = Bindings::register(&mut runtime, &image, 100).unwrap();
        let commodity_bindings =
            crate::commodity_script::Bindings::register(&mut runtime, &image, 200).unwrap();
        let commodity_source = std::fs::read(crate::fixtures::path("COMMODITY_SETTINGS")).unwrap();
        runtime
            .execute(
                "commoditysettings.lua",
                &commodity_source,
                &mut |id, args| commodity_bindings.handle(id, args, &mut commodities),
            )
            .unwrap();
        runtime
            .execute("businesssettings.lua", &source, &mut |id, args| {
                bindings.handle(id, args, &mut state, &commodities)
            })
            .unwrap();
        assert_eq!(&native[..8], b"MTCAT001");
        assert_eq!(
            f32::from_le_bytes(state.info.row(2).unwrap()[0x58..0x5c].try_into().unwrap()),
            3.,
            "original DELI stockcapacity assignment"
        );
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        let mut at = 8;
        for (index, row) in state.info.rows().iter().enumerate() {
            let count = word(at) as usize;
            assert_eq!(usize::from(row[0x4c]), count);
            assert_eq!(
                u32::from_le_bytes(row[0x70..0x74].try_into().unwrap()),
                word(at + 4)
            );
            at += 8;
            for item in &state.products[index][..count] {
                assert_eq!(*item, [word(at), word(at + 4)]);
                at += 8;
            }
        }
        assert_eq!(at, native.len());
        let bakery = state.stock_products(0, &commodities).unwrap();
        assert!(!bakery.is_empty());
        assert!(bakery.iter().all(|p| p.value > 0.));
        let input = crate::building_binding::ConstructionInputs {
            precision: crate::x87::Precision::Pc64,
            volume: 27,
            kind: 0,
            quality: 1,
            category_value: 0.5,
            hour: 9,
            discount: 0.1,
            owner: 1,
            utility_owners: [0; 4],
            business: state.info.row(0).unwrap(),
            products: &bakery,
        };
        let cost = crate::building_binding::construction_cost(&input, &image).unwrap();
        assert!(cost.total > 0 && cost.components[1] > 0.);
        let properties =
            std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_PROPERTIES")).unwrap();
        assert_eq!(&properties[..8], b"MTBIP001");
        let word = |at| u32::from_le_bytes(properties[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 45);
        for (index, row) in state.info.rows().iter().enumerate() {
            let at = 16 + index * 120;
            assert_eq!(word(at) as usize, index);
            let mut actual = *row;
            // The property oracle intentionally does not execute AddCommodity.
            actual[0x4c] = 0;
            actual[0x70..0x74].fill(0);
            assert_eq!(
                &actual,
                &properties[at + 4..at + 120],
                "native properties row {index}"
            );
        }
        let mut at = 16 + 45 * 120;
        for _ in 0..word(12) {
            let name = match word(at) {
                1 => "minarea",
                2 => "openingtime",
                3 => "density",
                4 => "available",
                _ => panic!("unknown property primitive"),
            };
            let value = f64::from_le_bytes(properties[at + 4..at + 12].try_into().unwrap());
            let offset = word(at + 12) as usize;
            let mut fresh = State::from_v14(&image).unwrap();
            runtime.set_number("edge_value", value).unwrap();
            runtime
                .execute(
                    "property conversion edge",
                    format!("BusinessInfo[0].{name}=edge_value").as_bytes(),
                    &mut |id, args| bindings.handle(id, args, &mut fresh, &commodities),
                )
                .unwrap();
            assert_eq!(
                &fresh.info.row(0).unwrap()[offset..offset + 4],
                &properties[at + 16..at + 20],
                "{name} {value}"
            );
            at += 20;
        }
        assert_eq!(at, properties.len());
        // Independent supplied save corroborates configured scalar settings;
        // availability can be changed by scenario scripts after DEFAULT.
        let saved = std::fs::read(crate::fixtures::path("SAVE_ONE")).unwrap();
        let save = crate::save::SaveFile::parse(&saved).unwrap();
        let saved_info = save
            .sections
            .iter()
            .find(|s| s.tag == 0x40000)
            .unwrap()
            .payload;
        for (index, row) in state.info.rows().iter().enumerate() {
            for range in [0x34..0x4d, 0x50..0x5f, 0x60..0x74] {
                assert_eq!(
                    &row[range.clone()],
                    &saved_info[index * 0x74 + range.start..index * 0x74 + range.end],
                    "business {index}, offset {}",
                    range.start
                );
            }
        }
        assert_eq!(
            state.info.rows().iter().filter(|r| r[0x30] != 0).count(),
            25
        );
        runtime.execute("native conversions", b"BusinessInfo[BAKERY].openingtime=255.9; BusinessInfo[BAKERY].minarea=-2.9; a=BusinessInfo[BAKERY].openingtime; b=BusinessInfo[BAKERY].minarea; c=BusinessInfo[APARTMENT].type", &mut |id, args| bindings.handle(id, args, &mut state, &commodities)).unwrap();
        assert_eq!(runtime.number("a").unwrap(), Some(-1.));
        assert_eq!(runtime.number("b").unwrap(), Some(-2.));
        assert_eq!(runtime.number("c").unwrap(), Some(42.));
    }
}
