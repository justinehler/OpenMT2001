//! Original strategy continuation actions47bc50..47c390 and47b1a0.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub arguments: [u32; 3],
    pub count: usize,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub value: u32,
    pub outputs: [u32; 2],
}
pub struct World<'a> {
    pub hour: i32,
    pub cash_statuses: &'a [u8; 6],
    pub targets: &'a mut std::collections::BTreeMap<u32, [u8; 12]>,
}
fn defaults(player: &mut Player, image: &PeImage<'_>) -> Result<(), String> {
    let name = image.at(0x53edb8, 0x104)?;
    let len = name
        .iter()
        .position(|&x| x == 0)
        .ok_or("AI action default name unterminated")?
        + 1;
    player.bytes[0xe3c..0xe3c + len].copy_from_slice(&name[..len]);
    player.bytes[0xf40..0x1048].copy_from_slice(image.at(0x53fa18, 264)?);
    player.bytes[0x1048..0x1150].copy_from_slice(image.at(0x540570, 264)?);
    Ok(())
}
pub fn advance(
    address: u32,
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    image: &PeImage<'_>,
    world: &mut World<'_>,
    mut callback: impl FnMut(
        Call,
        &mut Player,
        &mut [[u8; 0x650]],
        &mut World<'_>,
    ) -> Result<Reply, String>,
) -> Result<(), String> {
    macro_rules! call {
        ($at:expr,$args:expr) => {{
            let args = &$args;
            let mut full = [0; 3];
            full[..args.len()].copy_from_slice(args);
            callback(
                Call {
                    address: $at,
                    arguments: full,
                    count: args.len(),
                },
                player,
                profiles,
                world,
            )?
        }};
    }
    let mut retire = false;
    let mut delay = false;
    match address {
        0x47b1a0 => retire = true,
        0x47bc50 => {
            let cash = f32::from_bits(word(&player.bytes, 4));
            let reserve = f32::from_bits(word(&player.bytes, 0xcac));
            if cash.is_nan() || reserve.is_nan() || cash < reserve {
                retire = true;
            } else if player.bytes[0xb84] != 0 {
                player.bytes[0xb84] = 0;
                retire = true;
            } else if player.bytes[0xb76] != 0 {
                if player.bytes[0xb78] != 0 || player.bytes[0xb79] != 0 {
                    call!(0x46b6c0, [word(&player.bytes, 0xb68)]);
                    retire = true;
                } else {
                    retire = f64::from(word(&player.bytes, 0xb60) as i32) <= f64::from(cash)
                        || cash.is_nan();
                }
            } else {
                let windows = crate::ai_selection::windows(player, profiles, world.hour)?;
                retire = match word(&player.bytes, 0x1040) {
                    0 => !windows.0,
                    1 => !windows.1,
                    _ => false,
                };
            }
        }
        0x47be10 => {
            retire =
                crate::ai_selection::windows(player, profiles, world.hour)?.0 || world.hour == 6
        }
        0x47bed0 => {
            let amount = word(&player.bytes, 0xb88) as i32;
            let income = f32::from_bits(word(&player.bytes, 0xca4));
            let cash = f32::from_bits(word(&player.bytes, 4));
            let stop = f64::from(amount) > f64::from(income)
                || f64::from(amount) <= f64::from(cash)
                || cash.is_nan()
                || word(&player.bytes, 0xb8c) as i32 > 48;
            let count = call!(0x4727d0, [player.reference]).value as i32;
            let mut recovered = false;
            if player.bytes[0x1402] != 0 {
                let owner = player.bytes[0x1bc] as usize;
                let status = *world
                    .cash_statuses
                    .get(owner)
                    .ok_or("AI continuation cash status leaves table")?;
                let hours = if status != 0 { 24i32 } else { 48i32 };
                let hours = hours.wrapping_sub(world.hour);
                let mut projected = Sum::from_f32(f32::from_bits(word(&player.bytes, 0xca4)))?;
                projected.mul_f32(image.f32_at(0x525cec)?)?;
                let projected = projected.ftol_low_word().wrapping_mul(hours);
                let required = call!(0x47d990, [player.reference]).value as i32;
                let mut funds = Sum::from_i32(projected);
                funds.add_f32(f32::from_bits(word(&player.bytes, 4)))?;
                if Sum::from_i32(required).compare(funds) == std::cmp::Ordering::Greater
                    && count >= 1
                {
                    recovered = count <= call!(0x47d700, [player.reference]).value as i32;
                }
            }
            retire = stop || recovered;
            delay = retire;
        }
        0x47c030 => {
            let category = word(&player.bytes, 0xcbc) as i32;
            if !(0..=52).contains(&category) {
                retire = true;
            } else {
                if player.bytes[0xb74] != 0 && word(&player.bytes, 0xb70) as i32 > 0 {
                    let value = word(&player.bytes, 0xb70).wrapping_sub(1);
                    put(&mut player.bytes, 0xb70, value);
                }
                let checked = call!(0x4698c0, [category as u32, u32::MAX, 0]).value as u8 != 0;
                if checked
                    || word(&player.bytes, 0xcbc) == u32::MAX
                    || (player.bytes[0xb74] != 0 && word(&player.bytes, 0xb70) == 0)
                {
                    retire = true;
                } else if player.bytes[0xcb4] != 0 {
                    if word(&player.bytes, 0x240) == 7 {
                        put(&mut player.bytes, 0xb64, 2);
                    } else if player.bytes[0xb76] != 0 {
                        put(&mut player.bytes, 0xb68, u32::MAX);
                        player.bytes[0xb76] = 0;
                        let mut action = call!(0x47aea0, [player.reference, u32::MAX]).outputs[0];
                        if word(&player.bytes, 0xb68) as i32 <= -1 {
                            let windows =
                                crate::ai_selection::windows(player, profiles, world.hour)?;
                            let initial = call!(0x4739f0, [player.reference, 0]);
                            if initial.value as i32 > -1 {
                                put(&mut player.bytes, 0xb64, initial.outputs[0]);
                                if initial.outputs[0] == 4 {
                                    player.bytes[0xc97] = 1;
                                }
                                put(&mut player.bytes, 0xb68, initial.value);
                                put(&mut player.bytes, 0xb54, 0);
                                if windows.0 {
                                    action = 0;
                                } else if windows.1 {
                                    action = 1;
                                }
                            }
                        }
                        if word(&player.bytes, 0xb68) as i32 > -1 {
                            player.bytes[0xcc0] = 1;
                            match action {
                                0 => {
                                    player.bytes[0xb58] = 1;
                                    put(&mut player.bytes, 0x1040, 0);
                                }
                                1 => {
                                    player.bytes[0xb58] = 0;
                                    put(&mut player.bytes, 0x1040, 1);
                                }
                                _ => {
                                    call!(0x4e6960, [1, 0, 0x54070c]);
                                }
                            }
                        }
                    }
                }
            }
        }
        0x47c2d0 => {
            retire = player.bytes[0xb76] != 0;
            if !retire {
                let category = word(&player.bytes, 0xb68);
                retire = *player
                    .bytes
                    .get(0x178 + category as usize)
                    .ok_or("AI continuation ownership leaves player")?
                    == 0;
                if !retire {
                    retire = call!(0x477860, [category]).value as u8 != 0;
                }
            }
        }
        0x47c390 => {
            let required = call!(0x47d990, [player.reference]).value as i32;
            let cash = f32::from_bits(word(&player.bytes, 4));
            if (f64::from(cash) <= f64::from(required) || cash.is_nan())
                && word(&player.bytes, 0x13ec) as i32 > 0
            {
                let target = call!(0x4753f0, [player.reference]).value;
                if target == 0 {
                    retire = true;
                } else {
                    let record = world
                        .targets
                        .get(&target)
                        .ok_or("AI continuation target reference unresolved")?;
                    match word(record, 4) {
                        0 => retire = true,
                        1 => {
                            let category = u32::from(record[8]);
                            call!(0x478570, [category]);
                            call!(0x46a8d0, [target]);
                        }
                        2 | 3 => {
                            let business = word(record, 8);
                            let category = call!(0x462f30, [business]).value as i16 as i32 as u32;
                            let business = word(
                                world
                                    .targets
                                    .get(&target)
                                    .ok_or("AI continuation live target missing")?,
                                8,
                            );
                            call!(0x478410, [business, category]);
                            call!(0x46a8d0, [target]);
                        }
                        _ => {
                            call!(0x4e6960, [1, 0, 0x540744]);
                        }
                    }
                }
            } else {
                retire = true;
            }
        }
        _ => return Err(format!("AI continuation action{address:08x} unresolved")),
    }
    if retire {
        call!(0x4d5d00, [0x53ca70, 0, word(&player.bytes, 0x150)]);
        put(&mut player.bytes, 0x240, 2);
        put(&mut player.bytes, 0x1e18, 0);
        player.bytes[0x1e13] = 0;
        crate::player_constructor::reset_business_state(&mut player.bytes);
        defaults(player, image)?;
        if delay {
            put(&mut player.bytes, 0x13e4, 12);
        }
    }
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
    #[ignore = "requires original continuation action observations"]
    fn matches_native_actions() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_ACTION_CONTINUATION").unwrap())
                .unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAACTC1");
        assert_eq!(r.word(), 0x37f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let address = r.word();
            let hour = r.word() as i32;
            let status = r.word() as u8;
            let count = r.word();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let mut profiles = [r.blob().try_into().unwrap()];
            let mut targets =
                std::collections::BTreeMap::from([(0x10040000, r.blob().try_into().unwrap())]);
            let pa = r.blob();
            let proa = r.blob();
            let ta = r.blob();
            let mut trace = vec![];
            for _ in 0..count {
                let at = r.word();
                let value = r.word();
                let outputs = [r.word(), r.word()];
                let n = r.word();
                let args: Vec<_> = (0..n).map(|_| r.word()).collect();
                trace.push((
                    at,
                    value,
                    outputs,
                    args,
                    r.blob(),
                    r.blob(),
                    r.blob(),
                    r.blob(),
                    r.blob(),
                    r.blob(),
                ));
            }
            let statuses = [0, status, 0, 0, 0, 0];
            let mut world = World {
                hour,
                cash_statuses: &statuses,
                targets: &mut targets,
            };
            let mut cursor = 0;
            advance(
                address,
                &mut player,
                &mut profiles,
                &image,
                &mut world,
                |call, player, profiles, world| {
                    let (at, value, outputs, args, pb, prof_b, tb, pa, prof_a, ta) = &trace[cursor];
                    cursor += 1;
                    assert_eq!(call.address, *at, "{name} callback{cursor}");
                    assert_eq!(
                        &call.arguments[..call.count],
                        args,
                        "{name} arguments{at:x}"
                    );
                    assert_eq!(player.bytes, *pb, "{name} player before{at:x}");
                    assert_eq!(
                        profiles[0].as_slice(),
                        *prof_b,
                        "{name} profile before{at:x}"
                    );
                    assert_eq!(
                        world.targets[&0x10040000].as_slice(),
                        *tb,
                        "{name} targets before{at:x}"
                    );
                    player.bytes.copy_from_slice(pa);
                    profiles[0].copy_from_slice(prof_a);
                    world.targets.insert(0x10040000, (*ta).try_into().unwrap());
                    Ok(Reply {
                        value: *value,
                        outputs: *outputs,
                    })
                },
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(cursor, trace.len(), "{name} complete trace");
            assert_eq!(player.bytes, pa, "{name} final player");
            assert_eq!(profiles[0].as_slice(), proa, "{name} final profiles");
            assert_eq!(
                world.targets[&0x10040000].as_slice(),
                ta,
                "{name} final targets"
            );
        }
        assert!(r.0.is_empty());
    }
}
