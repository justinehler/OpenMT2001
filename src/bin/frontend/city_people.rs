//! Original4112a0 city People report over immutable resident prefixes.
use openmt2001::{pe::PeImage, save::SaveFile, x87::Sum};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub style: u32,
    pub label: u32,
    pub value: i32,
}
#[derive(Clone)]
pub struct Report {
    pub initialized: bool,
    pub rows: Vec<Row>,
    pub markers: Vec<crate::people_markers::Marker>,
}
impl Report {
    pub fn from_world(
        world: &openmt2001::level_startup::World,
        image: &PeImage<'_>,
        players: &openmt2001::player_manager::Manager,
    ) -> Result<Self, String> {
        let count = world.ledgers.residents.active_count as usize;
        let nodes = world
            .ledgers
            .residents
            .nodes
            .get(..count)
            .ok_or("fresh People active count outside pool")?;
        let mut report = Self::from_nodes(nodes.iter().map(|r| r.as_bytes()), image)?;
        report.markers = crate::people_markers::from_world(world, image, players)?;
        report.initialized = true;
        Ok(report)
    }
    pub fn from_save(save: &SaveFile<'_>, image: &PeImage<'_>) -> Result<Self, String> {
        let mut report = Self::from_nodes(save.residents()?.nodes.into_iter(), image)?;
        report.markers = crate::people_markers::from_save(save, image)?;
        Ok(report)
    }
    pub fn from_nodes<'a>(
        nodes: impl Iterator<Item = &'a [u8; 0x14a]>,
        image: &PeImage<'_>,
    ) -> Result<Self, String> {
        Self::from_nodes_with_constants(nodes, image.f32_at(0x5250e0)?, image.f32_at(0x525058)?)
    }
    /// Live 4112a0 report values from the authoritative retained population.
    /// The two f32 literals are pinned v1.4 words, checked by the native test.
    pub fn live_rows(world: &openmt2001::level_startup::World) -> Result<Vec<Row>, String> {
        Self::live_rows_from_pool(&world.ledgers.residents)
    }
    fn live_rows_from_pool(
        residents: &openmt2001::economy::ResidentAccounts,
    ) -> Result<Vec<Row>, String> {
        let count = residents.active_count as usize;
        let nodes = residents
            .nodes
            .get(..count)
            .ok_or("live People active count outside pool")?;
        Ok(Self::from_nodes_with_constants(
            nodes.iter().map(|r| r.as_bytes()),
            0.01_f32,
            100.0_f32,
        )?
        .rows)
    }
    fn from_nodes_with_constants<'a>(
        nodes: impl Iterator<Item = &'a [u8; 0x14a]>,
        centi: f32,
        percent: f32,
    ) -> Result<Self, String> {
        let mut total = 0u32;
        let mut counts = [[0u32; 3]; 3];
        for node in nodes {
            total += 1;
            if total > 2000 {
                return Err("People report exceeds original resident capacity".into());
            }
            let word = |at| u32::from_le_bytes(node[at..at + 4].try_into().unwrap());
            let kind = word(0);
            let home = i16::from_le_bytes(node[14..16].try_into().unwrap());
            counts[0][if kind == 4 {
                2
            } else if home == -1 {
                1
            } else {
                0
            }] += 1;
            counts[1][if kind == 0 {
                0
            } else if kind == 3 {
                2
            } else {
                1
            }] += 1;
            counts[2][match word(60) {
                0 => 2,
                1 => 1,
                _ => 0,
            }] += 1;
        }
        let mut rows = vec![Row {
            style: 0,
            label: 1453,
            value: total as i32,
        }];
        for (group, (title, labels)) in counts.into_iter().zip([
            (1454, [1455, 1456, 1457]),
            (1458, [1459, 1460, 1461]),
            (1464, [1657, 1655, 1656]),
        ]) {
            rows.push(Row {
                style: 2,
                label: 0,
                value: 0,
            });
            rows.push(Row {
                style: 1,
                label: title,
                value: 0,
            });
            let mut previous_ratio = 0.;
            let mut previous_rounded = 0;
            for (i, (count, label)) in group.into_iter().zip(labels).enumerate() {
                // Original zero-population FDIV is unordered; FTOL's indefinite
                // result has low dword0. Avoid introducing a host NaN policy.
                let value = if total == 0 {
                    0
                } else {
                    let mut ratio = Sum::from_i32(count as i32);
                    ratio.div_f32(total as f32)?;
                    if i != 0 {
                        let mut error = Sum::from_i32(previous_rounded);
                        error.mul_f32(centi)?;
                        error.add_f32(-previous_ratio)?;
                        ratio.add(error.negated());
                    }
                    previous_ratio = ratio.to_f32();
                    ratio.mul_f32(percent)?;
                    ratio.add_f32(0.5)?;
                    previous_rounded = ratio.ftol_low_word();
                    previous_rounded
                };
                rows.push(Row {
                    style: 0,
                    label,
                    value,
                });
            }
        }
        Ok(Self {
            initialized: false,
            rows,
            markers: vec![],
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_resident_allocation_updates_live_city_count() {
        let mut residents = openmt2001::economy::ResidentAccounts::new_pool();
        assert_eq!(Report::live_rows_from_pool(&residents).unwrap()[0].value, 0);
        assert_eq!(residents.allocate_resident_slot().unwrap(), Some(0));
        // The original 45bb80 constructor increments active_count immediately
        // after the first-free 45bb10 allocator succeeds.
        residents.active_count = residents.active_count.wrapping_add(1);
        assert_eq!(Report::live_rows_from_pool(&residents).unwrap()[0].value, 1);
    }
    #[test]
    #[ignore = "requires original People report observations"]
    fn matches_native_city_people() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY_PEOPLE").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTCPOP01");
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        assert_eq!(
            image.f32_at(0x5250e0).unwrap().to_bits(),
            0.01_f32.to_bits()
        );
        assert_eq!(
            image.f32_at(0x525058).unwrap().to_bits(),
            100.0_f32.to_bits()
        );
        let mut at = 12;
        for case in 0..word(8) {
            let count = word(at) as usize;
            at += 4;
            let mut nodes = Vec::new();
            for _ in 0..count {
                let mut node = [0; 0x14a];
                node[..4].copy_from_slice(&b[at..at + 4]);
                node[14..16].copy_from_slice(&b[at + 4..at + 6]);
                node[60..64].copy_from_slice(&b[at + 6..at + 10]);
                nodes.push(node);
                at += 10;
            }
            let count = word(at);
            at += 4;
            let mut expected = Vec::new();
            for _ in 0..count {
                expected.push(Row {
                    style: word(at),
                    label: word(at + 4),
                    value: word(at + 8) as i32,
                });
                at += 12;
            }
            assert_eq!(
                Report::from_nodes(nodes.iter(), &image).unwrap().rows,
                expected,
                "case{case}"
            );
            assert_eq!(
                Report::from_nodes_with_constants(nodes.iter(), 0.01_f32, 100.0_f32)
                    .unwrap()
                    .rows,
                expected,
                "live case{case}"
            );
        }
        assert_eq!(at, b.len());
    }
}
