//! Original40f6c0 prestige ranking and display groups, over saved category values.
use openmt2001::{board::Board, save::SaveFile};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Space,
    Grade(i32),
    Block(usize),
}
#[derive(Clone, Debug)]
pub struct Prestige {
    pub rows: Vec<Row>,
    pub values: [f32; 52],
    pub overlay_categories: Vec<usize>,
}
impl Prestige {
    pub fn from_save(save: &SaveFile<'_>, board: &Board) -> Result<Self, String> {
        let tables: Vec<_> = save.sections.iter().filter(|s| s.tag == 4).collect();
        if tables.len() != 1 || tables[0].payload.len() != 0xb270 {
            return Err("invalid saved prestige table".into());
        }
        let table = tables[0].payload;
        let mut values = [0.; 52];
        let mut active = [0; 52];
        for (i, value) in values.iter_mut().enumerate() {
            *value = save.category_prestige(i)?;
            active[i] = table[0x9600 + i * 0x8c + 4];
        }
        Self::from_fields(active, values, board)
    }

    /// Save category prefixes are copied into runtime rows by 447d70; the
    /// +4 active flag and +84 prestige remain the native report inputs.
    pub fn from_runtime(
        world: &openmt2001::level_startup::World,
        board: &Board,
    ) -> Result<Self, String> {
        Self::from_fields(
            std::array::from_fn(|i| world.categories[i][4]),
            std::array::from_fn(|i| {
                f32::from_le_bytes(world.categories[i][0x84..0x88].try_into().unwrap())
            }),
            board,
        )
    }

    fn from_fields(active: [u8; 52], values: [f32; 52], board: &Board) -> Result<Self, String> {
        if values.iter().any(|value| value.is_nan()) {
            return Err("NaN prestige cannot be safely sorted by original swap loop".into());
        }
        // Original topology leaves unused slots40/45 flagged bit0.
        let mut flags = [1; 52];
        let mut names = [0; 52];
        for b in &board.blocks {
            flags[b.id] = b.flags;
            names[b.id] = b.id;
        }
        let mut order: Vec<_> = (0..52).collect();
        loop {
            let mut changed = false;
            for i in 0..51 {
                if values[order[i]] < values[order[i + 1]] {
                    order.swap(i, i + 1);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut grade = 5;
        let mut heading = false;
        let mut rows = Vec::new();
        for i in order {
            if active[i] != 1 || flags[i] & 1 != 0 || names[i] > 45 {
                continue;
            }
            // Original decreases once per eligible block, not once per crossed grade.
            if values[i] < (grade as f32) {
                rows.push(Row::Space);
                grade -= 1;
                heading = false;
            }
            if grade == 0 {
                break;
            }
            if !heading {
                rows.push(Row::Grade(grade));
                heading = true;
            }
            rows.push(Row::Block(i));
        }
        let overlay_categories = (0..52)
            .filter(|&i| ((active[i] == 1 && flags[i] & 1 == 0) || i == 41) && names[i] <= 45)
            .collect();
        Ok(Self {
            rows,
            values,
            overlay_categories,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original prestige fixture"]
    fn matches_native_city_prestige() {
        let fixture =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY_PRESTIGE").unwrap()).unwrap();
        assert_eq!(&fixture[..8], b"MTCPRES2");
        let mut at = 8;
        let read = |at: &mut usize| {
            let n = u32::from_le_bytes(fixture[*at..*at + 4].try_into().unwrap()) as usize;
            *at += 4;
            n
        };
        assert_eq!(read(&mut at), 3);
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let board = Board::from_v14(&openmt2001::pe::PeImage::parse(&exe).unwrap()).unwrap();
        for key in [
            "OPENMT2001_SAVE_ONE",
            "OPENMT2001_SAVE_TWO",
            "OPENMT2001_PRESTIGE_SYNTHETIC",
        ] {
            let bytes = std::fs::read(std::env::var(key).unwrap()).unwrap();
            let actual = Prestige::from_save(&SaveFile::parse(&bytes).unwrap(), &board).unwrap();
            at += 32;
            let count = read(&mut at);
            let expected: Vec<_> = (0..count)
                .map(|_| {
                    let kind = read(&mut at);
                    let value = read(&mut at);
                    match kind {
                        0 => Row::Space,
                        1 => Row::Grade(value as i32),
                        2 => Row::Block(value),
                        _ => panic!(),
                    }
                })
                .collect();
            assert_eq!(actual.rows, expected, "{key}");
            let count = read(&mut at);
            let expected: Vec<_> = (0..count).map(|_| read(&mut at)).collect();
            assert_eq!(
                actual.overlay_categories, expected,
                "{key} overlay categories"
            );
        }
        assert_eq!(at, fixture.len());
    }
}
