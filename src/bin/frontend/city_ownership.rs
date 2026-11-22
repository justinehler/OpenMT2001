//! Original40eb70 block ownership grouping from saved player flags/category74.
use openmt2001::save::SaveFile;
#[derive(Clone, Debug)]
pub struct Ownership {
    pub badges: [crate::ownership_badges::Badge; 52],
    pub groups: Vec<Vec<usize>>,
}
impl Ownership {
    /// Original 447d70 restores the saved +0..8c prefix into each runtime
    /// category. Player snapshot records likewise expose the runtime+4
    /// prefix, so the saved grouping rules apply to live fresh sessions.
    pub fn from_runtime(
        world: &openmt2001::level_startup::World,
        players: &crate::player_info::State,
    ) -> Result<Self, String> {
        let mut badges = [crate::ownership_badges::Badge {
            character: None,
            remaining: 0,
        }; 52];
        for (i, badge) in badges.iter_mut().enumerate() {
            let row = &world.categories[i];
            let owner = row[5] as i8;
            if owner != 0 {
                let player = players.records
                    .get(usize::try_from(owner - 1).map_err(|_| "negative category owner")?)
                    .ok_or("category owner leaves live roster")?;
                let character = u32::from_le_bytes(player[0x14c..0x150].try_into().unwrap())
                    as usize;
                if character >= 10 {
                    return Err("ownership badge character leaves original table".into());
                }
                badge.character = Some(character);
            }
            badge.remaining = i32::from_le_bytes(row[0x7c..0x80].try_into().unwrap());
        }
        Ok(Self {
            badges,
            groups: players.records.iter().map(|player|
                (0..52).filter(|&i| world.categories[i][0x74] != 0
                    && player[0x174 + i] != 0).collect()).collect(),
        })
    }
    pub fn from_save(save: &SaveFile<'_>) -> Result<Self, String> {
        let sections: Vec<_> = save.sections.iter().filter(|s| s.tag == 4).collect();
        if sections.len() != 1 || sections[0].payload.len() != 0xb270 {
            return Err("invalid saved category table for ownership".into());
        }
        let categories = sections[0].payload;
        Ok(Self {
            badges: crate::ownership_badges::Badge::from_save(save)?,
            groups: save
                .players()?
                .iter()
                .map(|player| {
                    (0..52)
                        .filter(|&i| {
                            categories[0x9600 + i * 0x8c + 0x74] != 0
                                && player.bytes[0x174 + i] != 0
                        })
                        .collect()
                })
                .collect(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original ownership fixture"]
    fn matches_native_city_ownership() {
        let fixture =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY_OWNERSHIP").unwrap()).unwrap();
        assert_eq!(&fixture[..8], b"MTCOWN01");
        let mut at = 8;
        let read = |at: &mut usize| {
            let n = u32::from_le_bytes(fixture[*at..*at + 4].try_into().unwrap()) as usize;
            *at += 4;
            n
        };
        assert_eq!(read(&mut at), 3);
        for key in [
            "OPENMT2001_SAVE_ONE",
            "OPENMT2001_SAVE_TWO",
            "OPENMT2001_OWNERSHIP_SYNTHETIC",
        ] {
            let bytes = std::fs::read(std::env::var(key).unwrap()).unwrap();
            let report = Ownership::from_save(&SaveFile::parse(&bytes).unwrap()).unwrap();
            at += 32;
            assert_eq!(read(&mut at), report.groups.len());
            for actual in report.groups {
                let count = read(&mut at);
                let expected: Vec<_> = (0..count).map(|_| read(&mut at)).collect();
                assert_eq!(actual, expected, "{key}");
            }
        }
        assert_eq!(at, fixture.len());
    }
}
