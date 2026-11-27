//! Original42b690 category rights overlay, relative to normal local-load player.
use openmt2001::save::SaveFile;
#[derive(Clone, Copy, Debug)]
pub struct Category {
    pub mode: i32,
    pub owner: u8,
    pub remaining: i32,
}
pub struct Rights {
    pub categories: [Category; 52],
    pub owner_number: u8,
    pub player_name: Vec<u8>,
}
impl Rights {
    pub fn from_runtime(
        world: &openmt2001::level_startup::World,
        players: &crate::player_info::State,
    ) -> Result<Option<Self>, String> {
        let Some(current) = players.current else {
            return Ok(None);
        };
        let player = players
            .records
            .get(current)
            .ok_or("rights current player leaves roster")?;
        let categories = std::array::from_fn(|i| {
            let row = &world.categories[i];
            Category {
                mode: i32::from_le_bytes(row[0x78..0x7c].try_into().unwrap()),
                owner: row[5],
                remaining: i32::from_le_bytes(row[0x80..0x84].try_into().unwrap()),
            }
        });
        if categories
            .iter()
            .any(|row| usize::from(row.owner) > players.records.len())
        {
            return Err("rights category owner outside live roster".into());
        }
        let bytes = &player[0x154..];
        let end = bytes
            .iter()
            .position(|&b| b == 0)
            .ok_or("rights player name not terminated")?;
        let player_name = bytes[..end]
            .iter()
            .map(|b| {
                if (32..=126).contains(b) {
                    b - 32
                } else {
                    b'?' - 32
                }
            })
            .collect();
        Ok(Some(Self {
            categories,
            owner_number: (current + 1) as u8,
            player_name,
        }))
    }
    pub fn from_save(save: &SaveFile<'_>) -> Result<Option<Self>, String> {
        let Some(player) = crate::saved_current_player::normal_load(save)? else {
            return Ok(None);
        };
        let players = save.players()?;
        let table = save
            .sections
            .iter()
            .find(|s| s.tag == 4)
            .ok_or("rights category table missing")?
            .payload;
        if table.len() != 0xb270 {
            return Err("rights category table size".into());
        }
        let mut categories = [Category {
            mode: 0,
            owner: 0,
            remaining: 0,
        }; 52];
        for (i, c) in categories.iter_mut().enumerate() {
            let row = &table[0x9600 + i * 0x8c..];
            c.mode = i32::from_le_bytes(row[0x78..0x7c].try_into().unwrap());
            c.owner = row[5];
            c.remaining = i32::from_le_bytes(row[0x80..0x84].try_into().unwrap());
            if c.owner as usize > players.len() {
                return Err("rights category owner outside roster".into());
            }
        }
        let bytes = &player.bytes[0x154..];
        let end = bytes
            .iter()
            .position(|&b| b == 0)
            .ok_or("rights player name not terminated")?;
        let player_name = bytes[..end]
            .iter()
            .map(|b| {
                if (32..=126).contains(b) {
                    b - 32
                } else {
                    b'?' - 32
                }
            })
            .collect();
        Ok(Some(Self {
            categories,
            owner_number: (player.slot + 1) as u8,
            player_name,
        }))
    }
}
#[derive(Debug)]
pub struct Badge {
    pub background: [f32; 4],
    pub color: u32,
    pub icon_rect: [f32; 4],
    pub icon_color: u32,
}
pub fn badge(mode: i32, owned: bool, point: [f32; 2], size: [u32; 2], tick: u32) -> Option<Badge> {
    if mode == 2 {
        return None;
    }
    let alpha = tick.wrapping_mul(10) & 255;
    let color = match mode {
        0 => {
            if owned {
                0xff00ff00
            } else {
                0xffff0000
            }
        }
        1 => (alpha << 24) | if owned { 0xff00 } else { 0xff0000 },
        _ => 0xffffff,
    };
    let icon_color = if mode == 0 || mode == 1 && owned {
        0xffffffff
    } else {
        (alpha << 24) | 0xffffff
    };
    Some(Badge {
        background: [
            point[0] - 15.,
            point[1] - 15.,
            point[0] + 15.,
            point[1] + 15.,
        ],
        color,
        icon_rect: crate::ownership_badges::icon_rect(point, size),
        icon_color,
    })
}
pub fn text_point(point: [f32; 2], remaining: i32) -> [f32; 2] {
    [
        point[0]
            + if remaining >= 100 {
                4.
            } else if remaining >= 10 {
                8.
            } else {
                12.
            },
        point[1] + 6.,
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original rights overlay oracle"]
    fn matches_native_city_rights() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY_RIGHTS").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTRIGHTS");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        let count = word(&mut at);
        for _ in 0..count {
            let mode = word(&mut at) as i32;
            let remaining = word(&mut at) as i32;
            let owner = word(&mut at);
            let tick = word(&mut at);
            let point = std::array::from_fn(|_| f32::from_bits(word(&mut at)) * 0.5);
            let rows = word(&mut at);
            let actual = badge(mode, owner == 1, point, [17, 19], tick);
            assert_eq!(
                rows,
                if actual.is_none() {
                    0
                } else if owner == 0 {
                    2
                } else {
                    3
                }
            );
            if let Some(a) = actual {
                assert_eq!(word(&mut at), 0);
                for v in a.background {
                    assert_eq!(word(&mut at), v.to_bits());
                }
                assert_eq!(word(&mut at), a.color);
                assert_eq!(word(&mut at), 1);
                let [l, t, r, b] = a.icon_rect;
                for v in [l, t, 1.5, l, b, 1.5, r, b, 1.5, r, t, 1.5] {
                    assert_eq!(word(&mut at), v.to_bits());
                }
                assert_eq!(word(&mut at), a.icon_color);
                if owner != 0 {
                    assert_eq!(word(&mut at), 2);
                    for v in text_point(point, remaining) {
                        assert_eq!(word(&mut at), v.to_bits());
                    }
                    assert_eq!(word(&mut at) as i32, remaining);
                }
            }
        }
        assert_eq!(at, b.len());
    }
}
