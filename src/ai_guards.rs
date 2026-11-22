//! Original47b870 auction-mode guard and47b8f0 financial viability test.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};

#[derive(Clone, Copy, Debug)]
pub struct Auction {
    /// Original auction+7d, independent of winner/participant fields.
    pub active: bool,
    /// Original global57e784 object+b00, not the current bid price.
    pub progress: i32,
}

pub fn auction(
    player: &mut Player,
    profiles: &[[u8; 0x650]],
    state: Option<Auction>,
) -> Result<bool, String> {
    if word(&player.bytes, 0x240) == 7 {
        player.bytes[0xcb4] = 0;
        return Ok(true);
    }
    let Some(state) = state.filter(|s| s.active) else {
        return Ok(false);
    };
    let profile = profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("auction guard character leaves profiles")?;
    let offset = (word(&player.bytes, 0x13f4) as usize)
        .checked_mul(36)
        .and_then(|n| n.checked_add(0x5be))
        .ok_or("auction guard profile offset overflow")?;
    let cap = i32::from(
        *profile
            .get(offset)
            .ok_or("auction guard difficulty leaves profile")?,
    );
    let limit = (word(&player.bytes, 0xcb8) as i32).wrapping_add(500);
    let cash = f32::from_bits(word(&player.bytes, 4));
    // FILD(i32) and FLD(f32) are exact here; f64 preserves the comparison.
    if state.progress < cap || f64::from(limit) >= f64::from(cash) {
        put(&mut player.bytes, 0x240, 7);
        return Ok(true);
    }
    Ok(false)
}

pub fn viable(player: &Player, image: &PeImage<'_>) -> Result<bool, String> {
    let owned = crate::ai_strategy_queries::owned_categories(player)?;
    let business_count = i32::from(u16::from_le_bytes(
        player.bytes[0xb44..0xb46].try_into().unwrap(),
    ));
    let reference = word(&player.bytes, 0x21c);
    let event = player
        .auxiliaries
        .iter()
        .find(|a| a.reference == reference)
        .ok_or("viability event-card auxiliary unresolved")?;
    let cash = f32::from_bits(word(&player.bytes, 4));
    let reserved = f32::from_bits(word(&player.bytes, 0x1430));
    let bonus = word(&player.bytes, 0xb50) as i32;
    let threshold = image.f32_at(0x525058)?;
    let below = if cash.is_finite() && reserved.is_finite() && threshold.is_finite() {
        let mut total = Sum::from_f32(cash)?;
        total.add_f32(-reserved)?;
        total.add(Sum::from_i32(bonus));
        total.compare(Sum::from_f32(threshold)?).is_lt()
    } else {
        let total = f64::from(cash) - f64::from(reserved) + f64::from(bonus);
        total < f64::from(threshold) || total.is_nan() || threshold.is_nan()
    };
    Ok(!below || word(&event.bytes, 0x18) != 0 || owned.wrapping_add(business_count) > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires original47b870/47b8f0 observations"]
    fn matches_native_guards() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_GUARDS").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAGRD01");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let before = r.blob();
            let mut player = Player {
                reference: 0x10010000,
                bytes: before.to_vec(),
                auxiliaries: vec![],
            };
            let profiles = [r.blob().try_into().unwrap()];
            let aux = r.blob();
            player.auxiliaries.push(crate::player_manager::Auxiliary {
                reference: word(before, 0x21c),
                bytes: aux[..48].try_into().unwrap(),
            });
            assert_eq!(r.word(), 15);
            let inputs: [u32; 15] = std::array::from_fn(|_| r.word());
            let after = r.blob();
            let expected = r.word();
            let result = match entry {
                0x47b870 => auction(
                    &mut player,
                    &profiles,
                    (inputs[13] != 0).then_some(Auction {
                        active: inputs[2] != 0,
                        progress: inputs[12] as i32,
                    }),
                ),
                0x47b8f0 => viable(&player, &image),
                _ => panic!("unknown guard"),
            }
            .unwrap();
            assert_eq!(u32::from(result), expected, "{name} result");
            assert_eq!(player.bytes, after, "{name} fullplayer");
            assert_eq!(player.auxiliaries[0].bytes, &aux[..48], "{name} auxiliary");
        }
        assert!(r.0.is_empty());
    }
}
