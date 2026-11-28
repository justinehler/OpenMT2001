//! Original401a80 Land Rent rows. Saved memberships and live links are distinct inputs.
use openmt2001::pe::PeImage;
#[derive(Clone, Debug)]
pub struct Business {
    pub slot: usize,
    pub owner: u8,
    pub label: u32,
    pub rent: u16,
}
#[derive(Clone, Debug)]
pub enum Row {
    Heading {
        owner: u8,
        landlord: bool,
    },
    Amount {
        label: u32,
        value: f32,
        rule: bool,
        heading: bool,
    },
    Business(Business),
    Blank,
}
pub fn rows(owner: u8, players: usize, businesses: &[Business]) -> Result<Vec<Row>, String> {
    if usize::from(owner) > players || businesses.iter().any(|b| usize::from(b.owner) > players) {
        return Err("land rent owner outside roster".into());
    }
    // At most32 category buildings with unsigned16 rents: every partial sum
    // is exactly representable in f32 (original stores after each addition).
    if businesses.len() > 32 {
        return Err("land rent category capacity".into());
    }
    let own: u32 = businesses
        .iter()
        .filter(|b| b.owner == owner && owner != 0)
        .map(|b| u32::from(b.rent))
        .sum();
    let collected: u32 = businesses
        .iter()
        .filter(|b| b.owner != owner || owner == 0)
        .map(|b| u32::from(b.rent))
        .sum();
    let mut result = vec![Row::Amount {
        label: 1391,
        value: collected as f32,
        rule: false,
        heading: false,
    }];
    if owner != 0 {
        result.push(Row::Amount {
            label: 1390,
            value: own as f32,
            rule: false,
            heading: false,
        });
    }
    result.push(Row::Blank);
    let order = std::iter::once(owner)
        .chain((1..=players as u8).filter(|&p| p != owner))
        .chain((owner != 0).then_some(0));
    for group in order {
        let matching: Vec<_> = businesses.iter().filter(|b| b.owner == group).collect();
        if matching.is_empty() {
            continue;
        }
        result.push(Row::Heading {
            owner: group,
            landlord: group == owner,
        });
        let total: u32 = matching.iter().map(|b| u32::from(b.rent)).sum();
        result.extend(matching.into_iter().map(|b| Row::Business(b.clone())));
        result.push(Row::Amount {
            label: if group == owner && owner != 0 {
                1390
            } else {
                66
            },
            value: total as f32,
            rule: true,
            heading: group == 0 && owner == 0,
        });
        result.push(Row::Blank);
    }
    Ok(result)
}
/// Resolve native +15c heads / building+44 links without changing category
/// headers: saved indices overlap live financial fields such as prestige+84.
pub fn runtime_memberships(
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
) -> Result<[Vec<usize>; 52], String> {
    openmt2001::building_pool::category_chains(categories, buildings, 0x591318)
}
/// Decode only saved membership indices; native runtime headers require links.
pub fn saved_memberships(categories: &[&[u8]]) -> Result<Vec<Vec<usize>>, String> {
    categories
        .iter()
        .map(|category| {
            let count = i32::from_le_bytes(category[8..12].try_into().unwrap());
            if !(0..=32).contains(&count) {
                return Err("property report saved category count".into());
            }
            Ok(category[12..12 + count as usize * 4]
                .chunks_exact(4)
                .map(|id| u32::from_le_bytes(id.try_into().unwrap()))
                .filter(|&id| id != u32::MAX)
                .map(|id| id as usize)
                .collect())
        })
        .collect()
}
pub fn from_records(
    categories: &[&[u8]],
    buildings: &[&[u8]],
    businesses: &[&[u8]],
    players: usize,
    image: &PeImage<'_>,
) -> Result<Vec<Vec<Row>>, String> {
    let memberships = saved_memberships(categories)?;
    from_memberships(
        categories,
        buildings,
        businesses,
        players,
        image,
        &memberships,
    )
}
/// Membership topology stays separate from unchanged native category headers.
pub fn from_memberships(
    categories: &[&[u8]],
    buildings: &[&[u8]],
    businesses: &[&[u8]],
    players: usize,
    image: &PeImage<'_>,
    memberships: &[Vec<usize>],
) -> Result<Vec<Vec<Row>>, String> {
    if categories.len() != memberships.len() {
        return Err("land rent membership categories".into());
    }
    categories
        .iter()
        .zip(memberships)
        .map(|(category, membership)| {
            let mut rows_in_order = Vec::new();
            for &slot in membership {
                let b = buildings.get(slot).ok_or("land rent building index")?;
                let business = i32::from_le_bytes(b[0x1c..0x20].try_into().unwrap());
                if business == -1 {
                    continue;
                }
                let b = businesses
                    .get(business as usize)
                    .ok_or("land rent business index")?;
                let kind = u32::from_le_bytes(b[4..8].try_into().unwrap());
                if kind >= 45 {
                    return Err("land rent business type".into());
                }
                rows_in_order.push(Business {
                    slot,
                    owner: b[0xa],
                    label: u32::from_le_bytes(
                        image.at(0x53a968 + kind * 0x74, 4)?.try_into().unwrap(),
                    ),
                    rent: u16::from_le_bytes(b[0x18..0x1a].try_into().unwrap()),
                });
            }
            rows(category[5], players, &rows_in_order)
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn events(rows: Vec<Row>) -> Vec<(u32, u32)> {
        let mut actual = vec![];
        for row in rows {
            match row {
                Row::Heading { owner, landlord } => {
                    actual.push((if landlord { 2 } else { 1 }, u32::from(owner)))
                }
                Row::Amount {
                    label,
                    value,
                    heading,
                    ..
                } => {
                    actual.push((if heading { 3 } else { 0 }, label));
                    actual.push((4, value.to_bits()));
                }
                Row::Business(b) => {
                    actual.extend([(0, b.label), (5, u32::from(b.rent)), (6, b.slot as u32)])
                }
                Row::Blank => {}
            }
        }
        actual
    }
    #[test]
    #[ignore = "private original Land Rent fixture"]
    fn matches_original_land_rent() {
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_LAND_RENT")).unwrap();
        assert_eq!(&b[..8], b"MTLRENT1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        for _ in 0..word(&mut at) {
            let owner = word(&mut at) as u8;
            let count = word(&mut at);
            let mut businesses = vec![];
            for slot in 0..count {
                let owner = word(&mut at) as u8;
                let kind = word(&mut at);
                let rent = word(&mut at) as u16;
                businesses.push(Business {
                    slot: slot as usize,
                    owner,
                    label: u32::from_le_bytes(
                        image
                            .at(0x53a968 + kind * 0x74, 4)
                            .unwrap()
                            .try_into()
                            .unwrap(),
                    ),
                    rent,
                });
            }
            let expected = (0..word(&mut at))
                .map(|_| (word(&mut at), word(&mut at)))
                .collect::<Vec<_>>();
            let actual = events(rows(owner, 3, &businesses).unwrap());
            assert_eq!(actual, expected, "owner{owner}");
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "private original saved Land Rent reports"]
    fn matches_original_saved_land_rent() {
        let path = openmt2001::fixtures::path("NATIVE_LAND_RENT");
        let b = std::fs::read(path.with_file_name("native-land-rent-saved.bin")).unwrap();
        assert_eq!(&b[..8], b"MTLRSAV1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        for _ in 0..word(&mut at) {
            let n = word(&mut at) as usize;
            let name = std::str::from_utf8(&b[at..at + n]).unwrap();
            at += n;
            let category = word(&mut at) as usize;
            let expected = (0..word(&mut at))
                .map(|_| (word(&mut at), word(&mut at)))
                .collect::<Vec<_>>();
            let path = ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"]
                .into_iter()
                .map(|key| openmt2001::fixtures::path(key))
                .find(|p| p.file_name().unwrap() == name)
                .unwrap();
            let bytes = std::fs::read(path).unwrap();
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let report = super::super::Report::from_save(&save, &image)
                .unwrap()
                .unwrap();
            assert_eq!(
                events(report.land_rent[category].clone()),
                expected,
                "{name} category{category}"
            );
        }
        assert_eq!(at, b.len());
    }
}
