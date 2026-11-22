//! Original471d90/471d50 vacant perimeter-footprint area cache.
pub fn area_range(category: &[u8; 380]) -> [i32; 2] {
    let occupancy: &[u8; 49] = category[0x9c..0xcd].try_into().unwrap();
    let mut result = [99, 0];
    let mut visit = |x: i32, y: i32, width: i32, height: i32| {
        if crate::building_pool::can_place(occupancy, [x, y, x + width - 1, y + height - 1]) {
            let area = width * height;
            result[0] = result[0].min(area);
            result[1] = result[1].max(area);
        }
    };
    for height in 1..=6 {
        for width in 1..=6 {
            for x in 0..7 - width {
                visit(x, 0, width, height);
            }
            for x in 0..7 - width {
                visit(x, 7 - height, width, height);
            }
            for y in height..7 - height {
                visit(0, y, width, height);
            }
            for y in height..7 - height {
                visit(7 - width, y, width, height);
            }
        }
    }
    result
}

///471d00 clears all52 slots, then scans developable category groups only.
pub fn refresh_all(
    ranges: &mut [[i32; 2]; 52],
    categories: &[[u8; 380]; 52],
    image: &crate::pe::PeImage<'_>,
) -> Result<(), String> {
    ranges.fill([0; 2]);
    for (i, category) in categories.iter().enumerate() {
        let group = image.at(0x5385a4 + i as u32 * 88, 1)?[0];
        if group != 0 && !matches!(group, 10 | 11) {
            ranges[i] = area_range(category);
        }
    }
    Ok(())
}

pub fn refresh_building(
    ranges: &mut [[i32; 2]; 52],
    building: usize,
    buildings: &[[u8; 916]],
    categories: &[[u8; 380]; 52],
) -> Result<(), String> {
    let row = buildings
        .get(building)
        .ok_or("AI footprint building leaves pool")?;
    let reference = crate::player_manager::word(row, 0x40);
    let offset = reference
        .checked_sub(0x617628)
        .ok_or("AI footprint category precedes pool")?;
    if offset % 380 != 0 {
        return Err("AI footprint category is unaligned".into());
    }
    let category = categories
        .get(offset as usize / 380)
        .ok_or("AI footprint category leaves pool")?;
    let identity = crate::player_manager::word(category, 0) as usize;
    let record = categories
        .get(identity)
        .ok_or("AI footprint category identity leaves pool")?;
    ranges[identity] = area_range(record);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original471d90 footprint range observations"]
    fn matches_native_ranges() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_SPACE").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTAISPC1");
        let count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        assert_eq!(data.len(), 12 + count * (380 + 8));
        for (i, row) in data[12..].chunks_exact(388).enumerate() {
            let expected = [
                i32::from_le_bytes(row[380..384].try_into().unwrap()),
                i32::from_le_bytes(row[384..].try_into().unwrap()),
            ];
            assert_eq!(
                area_range(row[..380].try_into().unwrap()),
                expected,
                "case{i}"
            );
        }
    }
}
