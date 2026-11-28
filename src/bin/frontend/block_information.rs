//! Original4044b0 ordinary selected-block information; saved records remain read-only.
use openmt2001::{save::SaveFile, x87::Sum};
#[path = "property_business.rs"]
pub mod business;
#[path = "land_rent.rs"]
pub mod land_rent;
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Label(u32),
    Number(i32),
    Owner(u8),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub heading: u32,
    pub value: Option<Value>,
}
pub fn rows(
    prestige: f32,
    bias: f64,
    mode: i32,
    owner: u8,
    current: u8,
    lease: i32,
) -> Result<Vec<Row>, String> {
    let mut p = Sum::from_f32(prestige)?;
    p.add(Sum::from_f64(bias)?);
    let mut result = vec![
        Row {
            heading: 27,
            value: Some(if owner == 0 {
                Value::Label(1116)
            } else {
                Value::Owner(owner)
            }),
        },
        Row {
            heading: 35,
            value: Some(Value::Number(p.ftol_low_word().min(5))),
        },
        Row {
            heading: 36,
            value: match mode {
                0 => Some(Value::Label(13)),
                1 => Some(Value::Label(if owner == current { 17 } else { 15 })),
                2 => Some(Value::Label(21)),
                _ => None,
            },
        },
    ];
    if owner != 0 {
        result.push(Row {
            heading: 1393,
            value: Some(Value::Number(lease)),
        });
    }
    Ok(result)
}
#[derive(Clone)]
pub struct Report {
    pub initialized: bool,
    pub land_rent: Vec<Vec<land_rent::Row>>,
    pub utilities: crate::utility_report::Report,
    pub utility_categories: [usize; 4],
    pub railroad_categories: [usize; 4],
    pub railroads: Option<crate::utility_report::Railroads>,
    pub owners: [u8; 52],
    pub leases: [i32; 52],
    pub categories: Vec<Option<Vec<Row>>>,
    pub names: Vec<Vec<u8>>,
    pub title_colors: [u32; 52],
    pub businesses: Option<business::Report>,
    pub lease_available: [bool; 52],
}
impl Report {
    #[cfg(any(test, feature = "test-support"))]
    pub fn verify_runtime(
        world: &openmt2001::level_startup::World,
        players: &crate::player_info::State,
        image: &openmt2001::pe::PeImage<'_>,
        scenario: &str,
        difficulty: usize,
    ) -> Self {
        let report = Self::from_runtime(world, players, image).unwrap().unwrap();
        assert!(report.initialized);
        for (i, row) in world.categories.iter().enumerate() {
            assert_eq!(report.owners[i], row[5]);
            assert_eq!(
                report.leases[i],
                i32::from_le_bytes(row[0x7c..0x80].try_into().unwrap())
            );
        }
        if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_PROPERTY_ORACLE") {
            let mut bytes = b"MTUFRESH".to_vec();
            bytes.extend((players.records.len() as u32).to_le_bytes());
            bytes.extend((players.current.unwrap() as u32).to_le_bytes());
            for p in &players.records {
                bytes.extend(p);
            }
            for r in &world.categories {
                bytes.extend(r);
            }
            for r in &world.buildings {
                bytes.extend(r);
            }
            for r in &world.ledgers.businesses {
                bytes.extend(r);
            }
            for r in world.catalog.info.rows() {
                bytes.extend(r);
            }
            for utility in 0..4 {
                for details in [false, true] {
                    for owner in 0..=players.records.len() {
                        let amounts = report.utilities.amounts(utility, details, owner).unwrap();
                        bytes.extend((amounts.len() as u32).to_le_bytes());
                        for v in amounts {
                            bytes.extend(v.to_le_bytes());
                        }
                    }
                }
            }
            bytes.extend((report.categories.iter().flatten().count() as u32).to_le_bytes());
            for (i, rows) in report.categories.iter().enumerate() {
                if let Some(rows) = rows {
                    let mut pairs = Vec::new();
                    for row in rows {
                        pairs.push((0u32, row.heading));
                        if let Some(value) = &row.value {
                            pairs.push(match value {
                                Value::Label(n) => (1, *n),
                                Value::Number(n) => (2, *n as u32),
                                Value::Owner(n) => (3, u32::from(*n)),
                            });
                        }
                    }
                    bytes.extend((i as u32).to_le_bytes());
                    bytes.extend((pairs.len() as u32).to_le_bytes());
                    for (kind, value) in pairs {
                        bytes.extend(kind.to_le_bytes());
                        bytes.extend(value.to_le_bytes());
                    }
                }
            }
            std::fs::write(format!("{prefix}-{scenario}-{difficulty}.bin"), bytes).unwrap();
        }
        report
    }
    pub fn available(&self, category: usize) -> bool {
        self.categories.get(category).is_some_and(Option::is_some)
            || self.utility_categories.contains(&category)
            || (self.railroads.is_some() && self.railroad_categories.contains(&category))
    }
    pub fn attach_railroads(
        &mut self,
        properties: &openmt2001::economy::PropertyIncome,
        freight: f32,
    ) -> Result<(), String> {
        if !self.initialized {
            return Err("saved railroad load totals are not established".into());
        }
        if self.railroad_categories.map(|id| self.owners[id]) != properties.owners {
            return Err("railroad report ownership snapshot differs from retained income".into());
        }
        let report = crate::utility_report::Railroads {
            totals: properties.totals,
            freight,
        };
        for station in 0..4 {
            report.amounts(station, true)?;
        }
        self.railroads = Some(report);
        Ok(())
    }

    pub fn from_save(
        save: &SaveFile<'_>,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Option<Self>, String> {
        let Some(current) = crate::saved_current_player::normal_load(save)? else {
            return Ok(None);
        };
        let players: Vec<_> = save.players()?.iter().map(|p| *p.bytes).collect();
        let table = save
            .sections
            .iter()
            .find(|s| s.tag == 4)
            .ok_or("block information categories missing")?
            .payload;
        if table.len() != 0xb270 {
            return Err("block information category size".into());
        }
        let categories: Vec<_> = table[0x9600..].chunks_exact(0x8c).collect();
        let mut report = Self::from_records(
            &categories,
            &players,
            current.slot,
            crate::utility_report::Report::from_save(save, image)?,
            image,
            false,
        )?;
        let economy = save.economy()?;
        let businesses: Vec<_> = economy.businesses().map(|b| b.bytes.as_slice()).collect();
        let buildings: Vec<_> = table[..0x9600].chunks_exact(64).collect();
        report.land_rent =
            land_rent::from_records(&categories, &buildings, &businesses, players.len(), image)?;
        Ok(Some(report))
    }
    pub fn from_runtime(
        world: &openmt2001::level_startup::World,
        players: &crate::player_info::State,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Option<Self>, String> {
        if players.records.len() != world.ledgers.players.len() {
            return Err("block information financial snapshot no longer matches roster".into());
        }
        let Some(current) = players.current else {
            return Ok(None);
        };
        let categories: Vec<_> = world.categories.iter().map(|b| b.as_slice()).collect();
        let mut report = Self::from_records(
            &categories,
            &players.records,
            current,
            crate::utility_report::Report::from_runtime(world, image)?,
            image,
            true,
        )?;
        let buildings: Vec<_> = world.buildings.iter().map(|b| b.as_slice()).collect();
        let businesses: Vec<_> = world
            .ledgers
            .businesses
            .iter()
            .map(|b| b.as_slice())
            .collect();
        let membership = land_rent::runtime_memberships(&world.categories, &world.buildings)?;
        report.land_rent = land_rent::from_memberships(
            &categories,
            &buildings,
            &businesses,
            players.records.len(),
            image,
            &membership,
        )?;
        report.businesses = Some(business::Report::from_runtime(
            world,
            players.records.len(),
            image,
        )?);
        Ok(Some(report))
    }
    fn from_records(
        category_records: &[&[u8]],
        players: &[[u8; 440]],
        current: usize,
        utilities: crate::utility_report::Report,
        image: &openmt2001::pe::PeImage<'_>,
        initialized: bool,
    ) -> Result<Self, String> {
        if category_records.len() != 52 || current >= players.len() {
            return Err("block information roster/categories mismatch".into());
        }
        let bias = f64::from_le_bytes(image.at(0x525050, 8)?.try_into().unwrap());
        let special = image
            .at(0x541eac, 32)?
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .collect::<Vec<_>>();
        let names = players
            .iter()
            .map(|p| {
                let b = &p[0x154..];
                let end = b
                    .iter()
                    .position(|&v| v == 0)
                    .ok_or("block landlord name not terminated")?;
                Ok(b[..end]
                    .iter()
                    .map(|&v| {
                        if (32..=126).contains(&v) {
                            v - 32
                        } else {
                            b'?' - 32
                        }
                    })
                    .collect())
            })
            .collect::<Result<Vec<Vec<u8>>, String>>()?;
        let mut categories = Vec::new();
        let mut title_colors = [0; 52];
        for block in openmt2001::board::Board::from_v14(image)?.blocks {
            title_colors[block.id] = block.pavement_color;
        }
        let mut owners = [0; 52];
        let mut leases = [0; 52];
        let mut lease_available = [false; 52];
        for (i, b) in category_records.iter().enumerate() {
            lease_available[i] = b[0x74] != 0;
            if b.len() >= 0x98 {
                title_colors[i] = u32::from_le_bytes(b[0x94..0x98].try_into().unwrap());
            }
            let owner = b[5];
            owners[i] = owner;
            leases[i] = i32::from_le_bytes(b[0x7c..0x80].try_into().unwrap());
            if owner as usize > names.len() {
                return Err("block landlord outside roster".into());
            };
            categories.push(if special.contains(&i) {
                None
            } else {
                Some(rows(
                    f32::from_le_bytes(b[0x84..0x88].try_into().unwrap()),
                    bias,
                    i32::from_le_bytes(b[0x78..0x7c].try_into().unwrap()),
                    owner,
                    current as u8 + 1,
                    i32::from_le_bytes(b[0x7c..0x80].try_into().unwrap()),
                )?)
            });
        }
        Ok(Self {
            initialized,
            categories,
            names,
            utilities,
            utility_categories: special[..4].try_into().unwrap(),
            land_rent: Vec::new(),
            railroad_categories: special[4..].try_into().unwrap(),
            railroads: None,
            owners,
            leases,
            title_colors,
            businesses: None,
            lease_available,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original block information oracle"]
    fn matches_native_block_information() {
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_BLOCK_INFORMATION")).unwrap();
        assert_eq!(&b[..8], b"MTBLKINF");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let n = word(&mut at);
        for _ in 0..n {
            let prestige = f32::from_bits(word(&mut at));
            let mode = word(&mut at) as i32;
            let owner = word(&mut at) as u8;
            let count = word(&mut at);
            let expected = (0..count)
                .map(|_| (word(&mut at), word(&mut at) as i32))
                .collect::<Vec<_>>();
            let mut actual = vec![];
            for row in rows(prestige, 0.005, mode, owner, 1, 123).unwrap() {
                actual.push((0, row.heading as i32));
                if let Some(v) = row.value {
                    actual.push(match v {
                        Value::Label(n) => (1, n as i32),
                        Value::Number(n) => (2, n),
                        Value::Owner(n) => (3, n as i32),
                    });
                }
            }
            assert_eq!(actual, expected, "{prestige} {mode} {owner}");
        }
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let special = image
            .at(0x541eac, 32)
            .unwrap()
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .collect::<Vec<_>>();
        assert_eq!(word(&mut at), 52);
        for i in 0..52 {
            assert_eq!(b[at] != 0 || b[at + 1] != 0, special.contains(&i));
            at += 2;
        }
        assert_eq!(at, b.len());
    }
}
