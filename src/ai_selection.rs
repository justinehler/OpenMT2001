//! Original47a590 strategic selection, retaining callback order, table copies,
//! native RNG and target writes. Unrecovered target algorithms are explicit.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
};

#[derive(Clone, Copy, Debug)]
pub struct Time {
    pub hour: i32,
    pub year: i32,
}
impl Time {
    ///470ad0: original calendar+8 is zero, including negative hours, but no
    /// hour after23. This is the year word, not the separate elapsed-day word.
    pub fn first_day(self) -> bool {
        self.year == 0 && self.hour <= 23
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub arguments: [u32; 2],
    pub count: usize,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub value: u32,
    pub output: u32,
}

///472670/4726b0, inclusive overlapping boundary at the middle hour.
pub fn windows(
    player: &Player,
    profiles: &[[u8; 0x650]],
    hour: i32,
) -> Result<(bool, bool), String> {
    let profile = profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("AI time profile missing")?;
    let offset = (word(&player.bytes, 0x13f4) as usize)
        .checked_mul(20)
        .and_then(|v| v.checked_add(0x618))
        .ok_or("AI time profile overflow")?;
    let row = profile
        .get(offset..offset + 12)
        .ok_or("AI time difficulty leaves profile")?;
    Ok((
        hour >= word(row, 0) as i32 && hour <= word(row, 4) as i32,
        hour >= word(row, 4) as i32 && hour <= word(row, 8) as i32,
    ))
}

pub fn select(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    time: Time,
    mut callback: impl FnMut(Call, &mut Player, &mut [[u8; 0x650]]) -> Result<Reply, String>,
) -> Result<(), String> {
    crate::player_constructor::reset_business_state(&mut player.bytes);
    let mut invoke =
        |address, arguments, count, player: &mut Player, profiles: &mut [[u8; 0x650]]| {
            callback(
                Call {
                    address,
                    arguments,
                    count,
                },
                player,
                profiles,
            )
        };
    invoke(0x466fa0, [0, 0], 1, player, profiles)?;
    invoke(0x476a70, [0, 0], 0, player, profiles)?;
    let (build_window, buy_window) = windows(player, profiles, time.hour)?;
    if word(&player.bytes, 0xc) as i32 > -1 {
        return Ok(());
    }
    let stage = invoke(0x467820, [0, 0], 0, player, profiles)?.value;
    let copy = |player: &mut Player, table: u32| -> Result<(), String> {
        let at = 0x53db40u32.wrapping_add(table.wrapping_mul(788));
        player.bytes[0xe3c..0x1150].copy_from_slice(image.at(at, 788)?);
        Ok(())
    };
    if player.bytes[0x1402] != 0 {
        let trouble = invoke(0x4727d0, [player.reference, 0], 1, player, profiles)?.value as i32;
        let possible =
            invoke(0x47b950, [player.reference, 0], 1, player, profiles)?.value as u8 != 0;
        if possible
            && trouble >= 1
            && trouble <= invoke(0x47d700, [player.reference, 0], 1, player, profiles)?.value as i32
        {
            put(&mut player.bytes, 0xb54, 4);
            return copy(player, 4);
        }
    }
    let cash = f32::from_bits(word(&player.bytes, 4));
    let reserved = f32::from_bits(word(&player.bytes, 0xcac));
    if cash < reserved || cash.is_nan() || reserved.is_nan() {
        put(
            &mut player.bytes,
            0xb88,
            crate::economy::ftol_low_word(f64::from(reserved)) as u32,
        );
        put(&mut player.bytes, 0xb8c, 0);
        put(&mut player.bytes, 0xb54, 1);
        return copy(player, 1);
    }
    let landscape = invoke(0x46c870, [0, 0], 0, player, profiles)?.value as i32;
    if landscape > -1
        && *player
            .bytes
            .get(0x178 + landscape as usize)
            .ok_or("AI landscape target leaves player")?
            != 0
    {
        put(&mut player.bytes, 0xb68, landscape as u32);
        put(&mut player.bytes, 0xb64, 5);
        return copy(player, 0);
    }
    if !build_window && !buy_window {
        return copy(player, 5 + u32::from(time.hour >= 6));
    }
    let mut table = 6;
    let mut finish = false;
    if stage == 1 {
        let mut selected = false;
        if time.first_day() && player.bytes[0x13dc] == 0 {
            let target = invoke(0x4739f0, [player.reference, 0], 2, player, profiles)?;
            if target.value as i32 > -1 {
                put(
                    &mut player.bytes,
                    0xb64,
                    if matches!(target.output, 3 | 4) {
                        target.output
                    } else {
                        0
                    },
                );
                put(&mut player.bytes, 0xb68, target.value);
                put(&mut player.bytes, 0xb54, 0);
                player.bytes[0x13dc] = 1;
                selected = true;
            }
        }
        if !selected {
            selected =
                invoke(0x47aa90, [player.reference, 0], 1, player, profiles)?.value as u8 != 0;
        }
        if !selected {
            selected = invoke(0x47aea0, [player.reference, u32::MAX], 2, player, profiles)?.value
                as u8
                != 0;
        }
        if !selected {
            let _ = windows(player, profiles, time.hour)?;
            let target = invoke(0x4739f0, [player.reference, 0], 2, player, profiles)?;
            if target.value as i32 > -1 {
                put(&mut player.bytes, 0xb64, target.output);
                if target.output == 4 {
                    player.bytes[0xc97] = 1;
                }
                put(&mut player.bytes, 0xb68, target.value);
                put(&mut player.bytes, 0xb54, 0);
                selected = true;
            }
        }
        if selected {
            table = 0;
            let rng = word(&player.bytes, 0x228)
                .wrapping_mul(word(&player.bytes, 0x224))
                .wrapping_add(word(&player.bytes, 0x22c));
            put(&mut player.bytes, 0x224, rng);
            player.bytes[0x13f8] = u8::from(((rng >> 16) & 0x7fff) % 101 > 50);
            let target = word(&player.bytes, 0xb68);
            let score = invoke(0x4776c0, [player.reference, target], 2, player, profiles)?.value;
            put(&mut player.bytes, 0x13fc, score);
            finish = true;
        }
    } else if stage == 2 && word(&player.bytes, 0x1e48) == 20 {
        if player.bytes[0x1e44] < 200 {
            player.bytes[0x1e44] += 1;
        }
        if invoke(0x47ab80, [player.reference, 0], 1, player, profiles)?.value as u8 != 0 {
            table = 0;
            finish = true;
        }
    }
    if finish {
        player.bytes[0x1e45] = 0;
        player.bytes[0x1e44] = 0;
    }
    copy(player, table)
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
    #[ignore = "requires original strategic selector observations"]
    fn matches_native_selection() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_SELECTION").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTSELE01");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let time = Time {
                hour: r.word() as i32,
                year: r.word() as i32,
            };
            assert_eq!(r.word(), 0x037f);
            let expected = r.word();
            let count = r.word();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let mut profiles = [r.blob().try_into().unwrap()];
            let after = r.blob();
            let pa = r.blob();
            let mut trace = vec![];
            for _ in 0..count {
                let header: [u32; 5] = std::array::from_fn(|_| r.word());
                let args: Vec<_> = (0..header[4]).map(|_| r.word()).collect();
                trace.push((header, args, r.blob(), r.blob(), r.blob(), r.blob()));
            }
            let mut cursor = 0;
            let mut replay =
                |address: u32, args: &[u32], player: &mut Player, profiles: &mut [[u8; 0x650]]| {
                    let (h, arguments, pb, profile_before, pafter, profile_after) = &trace[cursor];
                    cursor += 1;
                    assert_eq!(address, h[0], "{name} target");
                    assert_eq!(args, arguments, "{name} arguments");
                    assert_eq!(player.bytes, *pb, "{name} before{address:08x}");
                    assert_eq!(
                        profiles[0].as_slice(),
                        *profile_before,
                        "{name} profile before"
                    );
                    player.bytes.copy_from_slice(pafter);
                    profiles[0].copy_from_slice(profile_after);
                    Ok(Reply {
                        value: h[2],
                        output: h[3],
                    })
                };
            match entry {
                0x47a590 => select(
                    &mut player,
                    &mut profiles,
                    &image,
                    time,
                    |call, player, profiles| {
                        replay(
                            call.address,
                            &call.arguments[..call.count],
                            player,
                            profiles,
                        )
                    },
                )
                .unwrap(),
                0x47b6d0 => {
                    let original_profile = profiles;
                    let result = crate::ai_strategy::development(
                        &mut player,
                        &original_profile,
                        &image,
                        time.hour,
                        |call, player| {
                            let (address, args) = match call {
                                crate::ai_strategy::Call::Diagnostic(at) => {
                                    (0x4e6960, vec![1, 0, at])
                                }
                                crate::ai_strategy::Call::Script { address, character } => {
                                    (0x4d5d00, vec![address, 0, character])
                                }
                                _ => panic!("unexpected development call"),
                            };
                            replay(address, &args, player, &mut profiles).map(|r| r.value)
                        },
                    )
                    .unwrap();
                    assert_eq!(result, expected, "{name} return");
                }
                _ => panic!("unexpected selector entry"),
            }
            assert_eq!(cursor, trace.len(), "{name} complete trace");
            assert_eq!(player.bytes, after, "{name} full player");
            assert_eq!(profiles[0].as_slice(), pa, "{name} full profile");
        }
        assert!(r.0.is_empty());
    }
}
