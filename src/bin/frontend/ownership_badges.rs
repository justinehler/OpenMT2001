//! Original42b910 ownership badge sources and centered geometry.
use openmt2001::{pe::PeImage, save::SaveFile};
#[derive(Clone, Copy, Debug)]
pub struct Badge {
    pub character: Option<usize>,
    pub remaining: i32,
}
impl Badge {
    pub fn from_save(save: &SaveFile<'_>) -> Result<[Self; 52], String> {
        let players = save.players()?;
        let table = save
            .sections
            .iter()
            .find(|s| s.tag == 4)
            .ok_or("ownership category table missing")?
            .payload;
        if table.len() != 0xb270 {
            return Err("ownership category table length".into());
        }
        let mut badges = [Self {
            character: None,
            remaining: 0,
        }; 52];
        for (i, badge) in badges.iter_mut().enumerate() {
            let row = &table[0x9600 + i * 0x8c..];
            let owner = row[5] as i8;
            if owner != 0 {
                let player = players
                    .get(usize::try_from(owner - 1).map_err(|_| "negative category owner")?)
                    .ok_or("category owner leaves saved roster")?;
                let character =
                    u32::from_le_bytes(player.metadata[..4].try_into().unwrap()) as usize;
                if character >= 10 {
                    return Err("ownership badge character leaves original table".into());
                }
                badge.character = Some(character);
            }
            badge.remaining = i32::from_le_bytes(row[0x7c..0x80].try_into().unwrap());
        }
        Ok(badges)
    }
}
pub fn style(
    image: &PeImage<'_>,
    character: Option<usize>,
) -> Result<(u32, Option<usize>), String> {
    let word = |address| {
        Ok::<u32, String>(u32::from_le_bytes(
            image.at(address, 4)?.try_into().unwrap(),
        ))
    };
    if let Some(c) = character {
        if c >= 10 {
            return Err("ownership badge character".into());
        }
        Ok((
            word(0x541af0 + c as u32 * 40)?,
            Some(word(0x541af8 + c as u32 * 40)? as usize),
        ))
    } else {
        Ok((word(0x5362d0)?, None))
    }
}
pub fn rect(point: [f32; 2], radius: f32) -> [f32; 4] {
    [
        point[0] - radius,
        point[1] - radius,
        point[0] + radius,
        point[1] + radius,
    ]
}
pub fn icon_rect(point: [f32; 2], size: [u32; 2]) -> [f32; 4] {
    let half = size.map(|v| (v / 2) as f32);
    [
        point[0] - half[0],
        point[1] - half[1],
        point[0] + half[0],
        point[1] + half[1],
    ]
}
/// Original42bce8 count label, using measured short category-name width.
pub fn count_point(point: [f32; 2], width: f32, remaining: i32) -> [f32; 2] {
    let left = (f64::from(point[0]) - f64::from((width * 0.5).trunc())) as f32;
    let top = (f64::from(point[1]) + 19.) as f32;
    [
        (f64::from(left)
            + if remaining >= 100 {
                18.
            } else if remaining >= 10 {
                22.
            } else {
                26.
            }) as f32,
        (f64::from(top) - 14.) as f32,
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original ownership badge submissions"]
    fn matches_native_ownership_badges() {
        let bytes = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&bytes).unwrap();
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_OWNERSHIP_BADGES")).unwrap();
        assert_eq!(&b[..8], b"MTOWNBD2");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let count = word(&mut at);
        for _ in 0..count {
            let c = word(&mut at) as i32;
            let remaining = word(&mut at) as i32;
            let p = std::array::from_fn(|_| f32::from_bits(word(&mut at)) * 0.5);
            let size = std::array::from_fn(|_| word(&mut at));
            let width = f32::from_bits(word(&mut at));
            let (color, icon) = style(&image, (c >= 0).then_some(c as usize)).unwrap();
            for (i, radius) in [19., 15.].into_iter().enumerate() {
                let expected = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
                assert_eq!(rect(p, radius), expected);
                assert_eq!(word(&mut at), if i == 0 { color } else { 0x55000000 });
            }
            assert_eq!(word(&mut at) != 0, icon.is_some());
            if let Some(icon) = icon {
                let r = icon_rect(p, size);
                let expected = [
                    r[0], r[1], 1.5, r[0], r[3], 1.5, r[2], r[3], 1.5, r[2], r[1], 1.5,
                ];
                for v in expected {
                    assert_eq!(word(&mut at), v.to_bits());
                }
                assert_eq!(word(&mut at) as usize, icon);
                assert_eq!(word(&mut at), u32::MAX);
                assert_eq!(
                    count_point(p, width, remaining).map(f32::to_bits),
                    [word(&mut at), word(&mut at)]
                );
            }
        }
        let n = word(&mut at) as usize;
        let tab = &b[at..at + n];
        at += n;
        let n = word(&mut at);
        for _ in 0..n {
            let _text_id = word(&mut at);
            let len = word(&mut at) as usize;
            let width = word(&mut at) as i32;
            assert_eq!(
                crate::tooltip::measure(tab, &b[at..at + len]).unwrap(),
                width
            );
            at += len;
        }
        assert_eq!(at, b.len());
    }
}
