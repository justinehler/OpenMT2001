//! Original47b240 strategy selection and47b3e0 action initiation. Strategic
//! scoring and world transactions remain explicit synchronous dependencies.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuationCall {
    Viable,
    Prepare { category: u32, amount: u32 },
    Action(u32),
}

///467690: continue the selected strategy, or retire a nonviable player.
/// Callbacks see and may mutate the authoritative profile/player immediately.
pub fn continue_strategy(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    mut callback: impl FnMut(ContinuationCall, &mut Player, &mut [[u8; 0x650]]) -> Result<u32, String>,
) -> Result<(), String> {
    if word(&player.bytes, 0x1040) == u32::MAX {
        return Ok(());
    }
    if callback(ContinuationCall::Viable, player, profiles)? as u8 == 0 {
        let profile = profiles
            .get_mut(word(&player.bytes, 0x150) as usize)
            .ok_or("AI continuation profile missing")?;
        crate::ai_parameters::reset_profile(profile, &mut player.bytes[0x1400]);
        return Ok(());
    }
    if player.bytes[0xb86] == 0 && player.bytes[0xb85] != 0 {
        let category = word(&player.bytes, 0xb68);
        player.bytes[0xb86] = 1;
        callback(
            ContinuationCall::Prepare {
                category,
                amount: 100,
            },
            player,
            profiles,
        )?;
        player.bytes[0x13c5] = 1;
    }
    let address = match word(&player.bytes, 0x1040) {
        0 | 1 => 0x47bc50,
        2 => 0x47c030,
        3 => 0x47c2d0,
        4 => 0x47bed0,
        5 => 0x47be10,
        6 | 7 => 0x47c390,
        9 => 0x47b1a0,
        _ => return Ok(()),
    };
    callback(ContinuationCall::Action(address), player, profiles)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Select,
    DevelopmentStrategy,
    PrepareAuction,
    Script {
        address: u32,
        character: u32,
    },
    /// Original4477e0 followed immediately by FTOL; reply is its signed low word.
    CategoryPrice(u32),
    Finalize,
    Diagnostic(u32),
}

fn defaults(player: &mut Player, image: &PeImage<'_>) -> Result<(), String> {
    let name = image.at(0x53edb8, 0x104)?;
    let len = name
        .iter()
        .position(|&b| b == 0)
        .ok_or("AI strategy default name unterminated")?
        + 1;
    player.bytes[0xe3c..0xe3c + len].copy_from_slice(&name[..len]);
    player.bytes[0xf40..0x1048].copy_from_slice(image.at(0x53fa18, 264)?);
    player.bytes[0x1048..0x1150].copy_from_slice(image.at(0x540570, 264)?);
    Ok(())
}

fn idle(
    player: &mut Player,
    image: &PeImage<'_>,
    callback: &mut impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<(), String> {
    callback(
        Call::Script {
            address: 0x53ca70,
            character: word(&player.bytes, 0x150),
        },
        player,
    )?;
    put(&mut player.bytes, 0x240, 2);
    put(&mut player.bytes, 0x1e18, 0);
    player.bytes[0x1e13] = 0;
    crate::player_constructor::reset_business_state(&mut player.bytes);
    defaults(player, image)
}

///47b6d0 chooses development/buying action within the profile's time windows.
pub fn development(
    player: &mut Player,
    profiles: &[[u8; 0x650]],
    image: &PeImage<'_>,
    hour: i32,
    callback: impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<u32, String> {
    let window = crate::ai_selection::windows(player, profiles, hour)?;
    development_in_window(player, image, window, callback)
}

pub fn development_in_window(
    player: &mut Player,
    image: &PeImage<'_>,
    (build, buy): (bool, bool),
    mut callback: impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<u32, String> {
    match word(&player.bytes, 0xb64) {
        5 => {
            let category = word(&player.bytes, 0xb68) as usize;
            if *player
                .bytes
                .get(0x178 + category)
                .ok_or("AI development category leaves player")?
                == 0
            {
                callback(Call::Diagnostic(0x5406dc), player)?;
            }
            Ok(3)
        }
        2 => Ok(2),
        3 | 4 => {
            let category = word(&player.bytes, 0xb68) as i32;
            if !(0..=52).contains(&category) {
                callback(Call::Diagnostic(0x5406b0), player)?;
            }
            Ok(if build {
                0
            } else if buy {
                1
            } else {
                5
            })
        }
        _ if !build && !buy => Ok(5),
        _ => {
            idle(player, image, &mut callback)?;
            Ok(u32::MAX)
        }
    }
}

///467220. Modes0/1 bypass Lua;5 kills the tactical timetable,3/4 load it.
pub fn timetable(
    player: &mut Player,
    mode: u32,
    callback: &mut impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<(), String> {
    let address = match mode {
        0 | 1 => None,
        3 => Some(0x53ca58),
        4 => Some(0x53ca4c),
        5 => Some(0x53ca70),
        _ => {
            callback(Call::Diagnostic(0x53ca1c), player)?;
            return Ok(());
        }
    };
    if let Some(address) = address {
        callback(
            Call::Script {
                address,
                character: word(&player.bytes, 0x150),
            },
            player,
        )?;
    }
    player.bytes[0x1e10] = 0;
    put(&mut player.bytes, 0x1e18, 0);
    player.bytes[0x1e13] = 1;
    player.bytes[0x238] = 1;
    Ok(())
}

pub fn select(
    player: &mut Player,
    image: &PeImage<'_>,
    mut callback: impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<(), String> {
    callback(Call::Select, player)?;
    let table = match word(&player.bytes, 0xf3c) {
        0 => Some(callback(Call::DevelopmentStrategy, player)?),
        1 => Some(4),
        3 => Some(7),
        4 => {
            callback(Call::PrepareAuction, player)?;
            Some(6)
        }
        5 => Some(5),
        6 => Some(9),
        _ => None,
    };
    if let Some(table) = table {
        let at = 0x53f0d0u32.wrapping_add(table.wrapping_mul(264));
        player.bytes[0xf40..0x1048].copy_from_slice(image.at(at, 264)?);
    }
    let action = match word(&player.bytes, 0x1040) {
        0 | 1 => 0,
        2 | 3 | 5 => 9,
        4 => 4,
        6 => 7,
        7 => 6,
        9 => {
            idle(player, image, &mut callback)?;
            u32::MAX
        }
        _ => u32::MAX,
    };
    let at = 0x53fc28u32.wrapping_add(action.wrapping_mul(264));
    player.bytes[0x1048..0x1150].copy_from_slice(image.at(at, 264)?);
    Ok(())
}

pub fn initiate(
    player: &mut Player,
    image: &PeImage<'_>,
    mut callback: impl FnMut(Call, &mut Player) -> Result<u32, String>,
) -> Result<bool, String> {
    let action = word(&player.bytes, 0x1040);
    match action {
        9 => {
            idle(player, image, &mut callback)?;
            Ok(false)
        }
        5 => {
            put(&mut player.bytes, 0x240, 3);
            put(&mut player.bytes, 0x244, 3);
            timetable(player, 5, &mut callback)?;
            //470880 is literally RET4; it does not modify the requested state.
            Ok(false)
        }
        2 => {
            let category = word(&player.bytes, 0xb68) as i32;
            if category < 0 {
                return Ok(false);
            }
            if *player
                .bytes
                .get(0x178 + category as usize)
                .ok_or("AI action category leaves player")?
                != 0
                || word(&player.bytes, 0xcb8) as i32 <= 0
            {
                return Ok(false);
            }
            player.bytes[0xb85] = 1;
            let price = callback(Call::CategoryPrice(category as u32), player)?;
            put(&mut player.bytes, 0xb6c, price);
            put(&mut player.bytes, 0x244, 3);
            put(&mut player.bytes, 0x240, 4);
            let category = word(&player.bytes, 0xb68);
            put(&mut player.bytes, 0xcbc, category);
            Ok(true)
        }
        3 => {
            let category = word(&player.bytes, 0xb68) as i32;
            if category < 0 {
                idle(player, image, &mut callback)?;
                return Ok(false);
            }
            if *player
                .bytes
                .get(0x178 + category as usize)
                .ok_or("AI landscape category leaves player")?
                == 0
            {
                callback(Call::Diagnostic(0x540678), player)?;
            }
            put(&mut player.bytes, 0x240, 3);
            put(&mut player.bytes, 0x244, 3);
            timetable(player, 3, &mut callback)?;
            Ok(true)
        }
        0 | 1 => {
            if word(&player.bytes, 0xb68) == u32::MAX {
                return Ok(false);
            }
            put(&mut player.bytes, 0x240, 3);
            put(&mut player.bytes, 0x244, 3);
            player.bytes[0xb58] = u8::from(action == 0);
            timetable(player, action, &mut callback)?;
            Ok(true)
        }
        4 => {
            let money = f32::from_bits(word(&player.bytes, 0xcac));
            put(&mut player.bytes, 0x240, 3);
            put(&mut player.bytes, 0x244, 3);
            put(
                &mut player.bytes,
                0xb60,
                crate::economy::ftol_low_word(f64::from(money)) as u32,
            );
            timetable(player, 4, &mut callback)?;
            Ok(true)
        }
        6 | 7 => {
            let mode = word(&player.bytes, 0x1148);
            put(&mut player.bytes, 0x240, 3);
            put(&mut player.bytes, 0x244, 3);
            if mode != 7 {
                return Ok(false);
            }
            callback(Call::Finalize, player)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original467690 continuation observations"]
    fn matches_native_continuation() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_CONTINUATION")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTCONT01");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let config: [u32; 12] = std::array::from_fn(|_| r.word());
            assert_eq!(config[11], 0x037f);
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.take(0x1e50).to_vec(),
                auxiliaries: vec![],
            };
            let character = word(&player.bytes, 0x150) as usize;
            let mut profiles = vec![[0; 0x650]; character + 1];
            profiles[character].copy_from_slice(r.take(0x650));
            let after = r.take(0x1e50);
            let pa = r.take(0x650);
            let mut aux = [0; 48];
            put(&mut aux, 0x18, config[5]);
            player.auxiliaries.push(crate::player_manager::Auxiliary {
                reference: word(&player.bytes, 0x21c),
                bytes: aux,
            });
            let mut trace = vec![];
            for _ in 0..config[10] {
                let h: [u32; 6] = std::array::from_fn(|_| r.word());
                let args: Vec<_> = (0..h[4]).map(|_| r.word()).collect();
                trace.push((
                    h,
                    args,
                    r.take(0x1e50),
                    r.take(0x650),
                    r.take(0x1e50),
                    r.take(0x650),
                ));
            }
            let mut cursor = 0;
            continue_strategy(&mut player, &mut profiles, |call, player, profiles| {
                let (h, args, before, pb, after, pa) = &trace[cursor];
                cursor += 1;
                let (address, receiver, arguments) = match call {
                    ContinuationCall::Viable => (0x47b8f0, 0, vec![player.reference]),
                    ContinuationCall::Prepare { category, amount } => {
                        (0x4784c0, player.reference, vec![category, amount])
                    }
                    ContinuationCall::Action(address) => (address, player.reference, vec![]),
                };
                assert_eq!((address, receiver), (h[0], h[1]), "{name} target");
                assert_eq!(&arguments, args, "{name} args");
                assert_eq!(&player.bytes, before, "{name} before {call:?}");
                assert_eq!(&profiles[character].as_slice(), pb, "{name} profile before");
                if call == ContinuationCall::Viable {
                    assert_eq!(
                        u32::from(crate::ai_guards::viable(player, &image).unwrap()),
                        h[2] & 255,
                        "{name} actual viability"
                    );
                } else {
                    player.bytes.copy_from_slice(after);
                    profiles[character].copy_from_slice(pa);
                }
                Ok(h[2])
            })
            .unwrap();
            assert_eq!(cursor, trace.len(), "{name} complete trace");
            assert_eq!(player.bytes, after, "{name} player");
            assert_eq!(profiles[character].as_slice(), pa, "{name} profile");
        }
        assert!(r.0.is_empty());
    }
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
    #[ignore = "requires original47b240/47b3e0 strategy observations"]
    fn matches_native_strategy() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_STRATEGY")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTASTR02");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.take(0x1e50).to_vec(),
                auxiliaries: vec![],
            };
            let mut profile = r.take(0x650).to_vec();
            let mut trace = Vec::new();
            for _ in 0..r.word() {
                let target = r.word();
                let object = r.word();
                let argc = r.word();
                let args: Vec<_> = (0..argc).map(|_| r.word()).collect();
                let value = r.word();
                r.word();
                let before = r.take(0x1e50);
                let pb = r.take(0x650);
                let after = r.take(0x1e50);
                let pa = r.take(0x650);
                if target != 0x467220 {
                    trace.push((target, object, args, value, before, pb, after, pa));
                }
            }
            let after = r.take(0x1e50);
            let profile_after = r.take(0x650);
            let expected = r.word();
            let mut cursor = 0;
            let mut callback = |call: Call, player: &mut Player| {
                let (target, object, args, value, before, pb, after, pa) = &trace[cursor];
                cursor += 1;
                let (address, arguments) = match call {
                    Call::Select => (0x47a590, vec![player.reference]),
                    Call::DevelopmentStrategy => (0x47b6d0, vec![player.reference]),
                    Call::PrepareAuction => (0x4754b0, vec![player.reference]),
                    Call::Script { address, character } => (0x4d5d00, vec![address, 0, character]),
                    Call::CategoryPrice(category) => {
                        assert_eq!(*object, 0x617628 + category * 380);
                        (0x4477e0, vec![])
                    }
                    Call::Finalize => (0x474f40, vec![player.reference]),
                    Call::Diagnostic(address) => (0x4e6960, vec![1, 0, address]),
                };
                assert_eq!(address, *target, "{name} target");
                assert_eq!(&arguments, args, "{name} args");
                assert_eq!(&player.bytes, before, "{name} pre-callback {call:?}");
                assert_eq!(&profile, pb, "{name} profile");
                player.bytes.copy_from_slice(after);
                profile.copy_from_slice(pa);
                Ok(*value)
            };
            match entry {
                0x47b240 => select(&mut player, &image, &mut callback).unwrap(),
                0x47b3e0 => assert_eq!(
                    u32::from(initiate(&mut player, &image, &mut callback).unwrap()),
                    expected,
                    "{name} AL"
                ),
                _ => panic!("unknown strategy entry"),
            }
            assert_eq!(cursor, trace.len(), "{name} complete callbacks");
            assert_eq!(player.bytes, after, "{name} full player");
            assert_eq!(profile, profile_after, "{name} full profile");
        }
        assert!(r.0.is_empty());
    }
}
