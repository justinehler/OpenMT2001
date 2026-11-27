//! Original00466ca0 outer AI controller. Decision algorithms and event polling
//! remain explicit synchronous callbacks; profile and scheduling stores live here.
use crate::player_manager::{Player, put, word};

pub struct Controls {
    pub normal_tick: u32,
    /// Original822d48, separate within-day timetable clock.
    pub day_tick: u32,
    /// Byte858 on the original global565b9c object.
    pub event_gate: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub arguments: [u32; 2],
    pub count: usize,
}

pub fn advance(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    controls: &mut Controls,
    image: &crate::pe::PeImage<'_>,
    mut callback: impl FnMut(
        Call,
        &mut Player,
        &mut [[u8; 0x650]],
        &mut Controls,
    ) -> Result<u32, String>,
) -> Result<(), String> {
    if player.bytes.len() < 0x1e50 {
        return Err("truncated AI controller".into());
    }
    if player.bytes[0x1400] != 0 || player.bytes[0x1403] != 0 {
        return Ok(());
    }
    let mut call = |address,
                    arguments,
                    count,
                    player: &mut Player,
                    profiles: &mut [[u8; 0x650]],
                    controls: &mut Controls| {
        callback(
            Call {
                address,
                arguments,
                count,
            },
            player,
            profiles,
            controls,
        )
    };
    if player.bytes[0x1401] == 0 {
        call(0x466870, [0; 2], 0, player, profiles, controls)?;
    }
    let age = word(&player.bytes, 0x1e48);
    if (age as i32) < 20 {
        put(&mut player.bytes, 0x1e48, age.wrapping_add(1));
    }
    call(0x466fa0, [1, 0], 1, player, profiles, controls)?;
    if player.bytes[0x1402] != 0 {
        let cash = f32::from_bits(word(&player.bytes, 4));
        let threshold = image.f32_at(0x525040)?;
        if cash >= threshold {
            player.bytes[0x1402] = 0;
        }
    }
    if player.bytes[0x1df8] == 0 {
        let due = u16::from_le_bytes(player.bytes[0xb4c..0xb4e].try_into().unwrap());
        if controls.normal_tick >= u32::from(due) {
            let next = call(
                0x47d6a0,
                [player.reference, 0],
                1,
                player,
                profiles,
                controls,
            )?;
            player.bytes[0xb4c..0xb4e].copy_from_slice(&(next as u16).to_le_bytes());
            call(0x4779b0, [0, 0], 2, player, profiles, controls)?;
            call(
                0x47b870,
                [player.reference, 0],
                1,
                player,
                profiles,
                controls,
            )?;
            if word(&player.bytes, 0x240) == 2 {
                if call(
                    0x47b8f0,
                    [player.reference, 0],
                    1,
                    player,
                    profiles,
                    controls,
                )? as u8
                    == 0
                {
                    player.bytes[0x1400] = 1;
                    let character = word(&player.bytes, 0x150) as usize;
                    let profile = profiles
                        .get_mut(character)
                        .ok_or("AI character leaves profile table")?;
                    for at in [0, 0xc, 0x10, 0x28, 0x1c] {
                        put(profile, at, 0);
                    }
                    profile[4] = 0;
                    for at in [8, 0xf0, 0x410, 0x280] {
                        put(profile, at, u32::MAX);
                    }
                    profile[0x36..0x40].fill(0);
                } else {
                    call(0x47b240, [0; 2], 0, player, profiles, controls)?;
                    call(0x47b3e0, [0; 2], 0, player, profiles, controls)?;
                }
            } else {
                call(0x467690, [0; 2], 0, player, profiles, controls)?;
            }
        }
        if (matches!(word(&player.bytes, 0xb64), 3..=5) || player.bytes[0xcc0] != 0)
            && controls.normal_tick > word(&player.bytes, 0x1e0c)
        {
            let character = word(&player.bytes, 0x150) as usize;
            let index = word(&player.bytes, 0x13f4) as usize;
            let profile = profiles
                .get(character)
                .ok_or("AI character leaves delayed profile table")?;
            let at = index
                .checked_mul(20)
                .and_then(|n| n.checked_add(0x624))
                .ok_or("AI delayed profile offset overflow")?;
            let amount = u32::from_le_bytes(
                profile
                    .get(at..at + 4)
                    .ok_or("AI delayed entry leaves profile")?
                    .try_into()
                    .unwrap(),
            );
            put(
                &mut player.bytes,
                0x1e0c,
                controls.normal_tick.wrapping_add(amount.wrapping_mul(25)),
            );
            player.bytes[0x221] |= 0x10;
        }
        for address in [0x467370, 0x467440, 0x468670] {
            call(address, [0; 2], 0, player, profiles, controls)?;
        }
    }
    let countdown = word(&player.bytes, 8) as i32;
    if countdown > 0 {
        let next = countdown - 1;
        put(
            &mut player.bytes,
            8,
            if controls.event_gate && next > 1 {
                1
            } else {
                next as u32
            },
        );
    } else {
        call(0x4675f0, [0; 2], 0, player, profiles, controls)?;
    }
    let vtable = word(&player.bytes, 0)
        .checked_add(4)
        .ok_or("AI vtable overflow")?;
    let target = u32::from_le_bytes(image.at(vtable, 4)?.try_into().unwrap());
    call(target, [0; 2], 0, player, profiles, controls)?;
    Ok(())
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
    #[ignore = "requires original466ca0 controller observations"]
    fn matches_native_ai_tick() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_TICK")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIT466");
        assert_eq!(r.word(), 2);
        let count = r.word();
        for _ in 0..count {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let mut controls = Controls {
                day_tick: 0,
                normal_tick: r.word(),
                event_gate: r.word() != 0,
            };
            let mask = r.word();
            let mutations: [u32; 3] = std::array::from_fn(|_| r.word());
            let n = r.word();
            let returns: Vec<_> = (0..n).map(|_| (r.word(), r.word())).collect();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let expected_player = r.blob();
            let character = word(&player.bytes, 0x150) as usize;
            let mut profiles = vec![[0; 0x650]; character + 1];
            profiles[character].copy_from_slice(r.blob());
            let expected_profile = r.blob();
            let calls = r.word();
            let expected_trace: Vec<_> = (0..calls)
                .map(|_| {
                    let address = r.word();
                    r.word();
                    let n = r.word();
                    (address, (0..n).map(|_| r.word()).collect::<Vec<_>>())
                })
                .collect();
            let mut trace = Vec::new();
            advance(
                &mut player,
                &mut profiles,
                &mut controls,
                &image,
                |call, player, _, _| {
                    trace.push((call.address, call.arguments[..call.count].to_vec()));
                    if call.address == 0x466870 {
                        if mask & 1 != 0 {
                            put(&mut player.bytes, 0x220, mutations[0]);
                        }
                        if mask & 2 != 0 {
                            player.bytes[0x1402] = mutations[1] as u8;
                        }
                        if mask & 4 != 0 {
                            player.bytes[0x1df8] = mutations[2] as u8;
                        }
                    }
                    Ok(returns
                        .iter()
                        .find(|(address, _)| *address == call.address)
                        .map_or(0, |(_, value)| *value))
                },
            )
            .unwrap();
            assert_eq!(trace, expected_trace, "{name}");
            assert_eq!(player.bytes, expected_player, "{name}");
            assert_eq!(profiles[character], expected_profile, "{name}");
        }
        assert!(r.0.is_empty());
    }
}
