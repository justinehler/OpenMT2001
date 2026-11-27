//! Original47c630 business planning. Helpers are synchronous calls with live
//! player/profile/category state and explicit in/out planning variables.
pub mod planning;
use crate::{
    pe::PeImage,
    player_manager::{Player, word},
};

/// Symbolic references to original stack locals in `Call::arguments`.
/// These are never host or emulated addresses.
pub const AREA: u32 = 0xff000010;
pub const FLOORS: u32 = 0xff000018;
pub const HEIGHT: u32 = 0xff000020;
pub const WIDTH: u32 = 0xff000024;
pub const COUNT: u32 = 0xff00002c;
pub const POSITION: u32 = 0xff000034;
pub const FOOTPRINTS: u32 = 0xff00004c;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Planning {
    pub area: u32,
    pub floors: u32,
    pub height: u32,
    pub width: u32,
    pub count: u32,
    pub position: [u32; 2],
    pub footprints: [u32; 44],
    pub outputs: [u32; 2],
}
impl Default for Planning {
    fn default() -> Self {
        Self {
            area: 0,
            floors: 0,
            height: 0,
            width: 0,
            count: 0,
            position: [0; 2],
            footprints: [0; 44],
            outputs: [u32::MAX; 2],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub arguments: [u32; 10],
    pub count: usize,
}
impl Call {
    fn new(address: u32, arguments: &[u32]) -> Self {
        let mut copied = [0; 10];
        copied[..arguments.len()].copy_from_slice(arguments);
        Self {
            address,
            arguments: copied,
            count: arguments.len(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub value: u32,
    ///445720's result after the caller's FSTP32.
    pub quote: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub execute: u8,
    pub style_override: u8,
    pub style: u32,
}

/// Outputs retain caller priors until the original corresponding store. The
/// position prior is explicit because original stack bytes are uninitialized.
/// Category52 is admitted by the original range guard; callers must supply its
/// original adjacent record if that unsupported category is actually reached.
pub fn choose_from_catalog(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    categories: &[[u8; 0x17c]],
    catalog: &crate::business_pool::BusinessCatalog,
    input: Input,
    state: &mut Planning,
    mut callback: impl FnMut(
        Call,
        &mut Player,
        &mut [[u8; 0x650]],
        &[[u8; 0x17c]],
        &mut Planning,
    ) -> Result<Reply, String>,
) -> Result<bool, String> {
    macro_rules! call {
        ($at:expr,$args:expr) => {
            callback(Call::new($at, &$args), player, profiles, categories, state)?
        };
    }
    state.outputs[0] = u32::MAX;
    state.area = 0;
    state.floors = 0;
    state.count = 0;
    state.width = 0;
    state.height = 0;
    let category = word(&player.bytes, 0xb68) as i32;
    if !(0..=52).contains(&category) {
        return Ok(false);
    }
    if categories
        .get(category as usize)
        .ok_or("AI planning category52 storage unresolved")?[0x6c]
        != 0
    {
        return Ok(false);
    }
    if input.execute != 0 && call!(0x4769f0, []).value as u8 != 0 && player.bytes[0xc97] != 0 {
        let category = word(&player.bytes, 0xb68);
        if *player
            .bytes
            .get(0x178 + category as usize)
            .ok_or("AI planning ownership index leaves player")?
            != 0
            && call!(
                0x47de90,
                [player.reference, category, POSITION, WIDTH, HEIGHT]
            )
            .value as u8
                != 0
            && call!(
                0x445ef0,
                [
                    state.position[0],
                    state.position[1],
                    state.width,
                    state.height
                ]
            )
            .value as u8
                != 0
        {
            state.floors = 1;
            let style = call!(0x47dd90, [player.reference, word(&player.bytes, 0xb68)]).value;
            let quote = call!(
                0x445720,
                [
                    word(&player.bytes, 0xb68),
                    state.width.wrapping_mul(state.height),
                    2,
                    word(&player.bytes, 0xb90),
                    style,
                    player.reference
                ]
            )
            .quote;
            let success = call!(
                0x478650,
                [
                    word(&player.bytes, 0xb68),
                    2,
                    44,
                    style,
                    quote.to_bits(),
                    state.position[0],
                    state.position[1],
                    state.width,
                    state.height,
                    state.floors
                ]
            )
            .value as u8
                != 0;
            call!(0x4725f0, [word(&player.bytes, 0xb68), 44]);
            return Ok(success);
        }
    }
    state.area = 8;
    let kind = word(&player.bytes, 0xb90);
    let record = catalog
        .row(kind as usize)
        .ok_or("AI planning catalog type missing")?;
    let group = record[0x5e] as usize;
    let profile = profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("AI planning profile missing")?;
    let mut minimum = *profile
        .get(0x40 + group * 35)
        .ok_or("AI planning group leaves profile")? as u32;
    let max_floors = word(record, 0x44);
    let mut min_floors = word(record, 0x48);
    if kind == 42 {
        state.area = call!(0x474080, [word(&player.bytes, 0x150)]).value;
        state.floors = call!(0x474100, [word(&player.bytes, 0x150)]).value;
        if state.floors as i32 > max_floors as i32 {
            state.floors = max_floors;
        }
        if state.area == 0 {
            return Ok(false);
        }
    } else {
        state.area = call!(0x47dd40, [word(&player.bytes, 0x150), kind]).value;
        state.floors = call!(0x47db30, [word(&player.bytes, 0x150)]).value;
        if word(&player.bytes, 0xb90) != 44 {
            call!(0x476ee0, [AREA, FLOORS, player.reference]);
        }
    }
    if (state.area as i32) < minimum as i32 {
        state.area = minimum;
    }
    state.footprints.fill(0);
    let building_type = word(
        catalog
            .row(word(&player.bytes, 0xb90) as usize)
            .ok_or("AI planning live catalog type missing")?,
        0xc,
    );
    let style = if input.style_override != 0 {
        input.style
    } else {
        call!(0x47dd90, [player.reference, word(&player.bytes, 0xb68)]).value
    };
    if word(&player.bytes, 0xb90) == 44 {
        minimum = 1;
        state.area = call!(0x4778c0, [word(&player.bytes, 0xb68)]).value;
        if state.area as i32 > 12 {
            state.area = 12;
        }
        state.width = 3;
        state.height = 4;
        min_floors = 1;
        state.floors = 1;
    }
    let mut tries = 0;
    while state.area as i32 >= minimum as i32 && state.floors as i32 >= min_floors as i32 {
        tries += 1;
        state.count = 0;
        if tries > 20 {
            return Ok(false);
        }
        state.footprints.fill(0);
        call!(0x46ba80, [word(&player.bytes, 0xb90), FOOTPRINTS, COUNT]);
        // Every failed iteration sets one of the two native retry flags.
        call!(0x46bd50, [state.area, FOOTPRINTS, COUNT]);
        state.width = state.footprints[0];
        state.height = state.footprints[1];
        let mut index = 0;
        let fit = loop {
            let fit = call!(
                0x4747c0,
                [
                    word(&player.bytes, 0xb68),
                    POSITION,
                    WIDTH,
                    HEIGHT,
                    u32::from(player.bytes[0x1bc])
                ]
            )
            .value as u8
                != 0;
            if fit || index as i32 > (state.count as i32).wrapping_sub(1) {
                break fit;
            }
            let pair = state
                .footprints
                .get(index * 2..index * 2 + 2)
                .ok_or("AI planning footprint count leaves native44-word buffer")?;
            state.width = pair[0];
            state.height = pair[1];
            index += 1;
        };
        if fit {
            let quote = call!(
                0x445720,
                [
                    word(&player.bytes, 0xb68),
                    state
                        .width
                        .wrapping_mul(state.height)
                        .wrapping_mul(state.floors),
                    building_type,
                    word(&player.bytes, 0xb90),
                    style,
                    player.reference
                ]
            )
            .quote;
            state.outputs[0] = 1;
            let integer = crate::economy::ftol_low_word(f64::from(quote)) as u32;
            state.outputs[1] = integer;
            let admitted = call!(0x48a6d0, [quote.to_bits(), 1]).value as u8 != 0;
            let cash = f32::from_bits(word(&player.bytes, 4));
            let reserve = f32::from_bits(word(&player.bytes, 0x1430));
            let minimum_cash = f32::from_bits(word(&player.bytes, 0xcac));
            let affordable = if !admitted
                || cash.is_nan()
                || reserve.is_nan()
                || quote.is_nan()
                || minimum_cash.is_nan()
            {
                false
            } else {
                let mut remaining = crate::x87::Sum::from_f32(cash)?;
                remaining.add_f32(-reserve)?;
                remaining.add_f32(-quote)?;
                remaining.compare(crate::x87::Sum::from_f32(minimum_cash)?)
                    != std::cmp::Ordering::Less
            };
            if affordable && call!(0x476b30, [integer, player.reference]).value as u8 == 0 {
                if input.execute == 0 {
                    return Ok(true);
                }
                if word(&player.bytes, 0xb90) == 44 {
                    let category = word(&player.bytes, 0xb68);
                    if *player
                        .bytes
                        .get(0x178 + category as usize)
                        .ok_or("AI planning landscape ownership leaves player")?
                        == 0
                    {
                        call!(
                            0x50e94f,
                            [
                                0xff0000fc,
                                0x54077c,
                                player.reference + 0x158,
                                0x5385a5u32.wrapping_add(category.wrapping_mul(0x58))
                            ]
                        );
                        call!(0x4e6960, [1, 0, 0xff0000fc]);
                    }
                }
                let success = call!(
                    0x478650,
                    [
                        word(&player.bytes, 0xb68),
                        building_type,
                        word(&player.bytes, 0xb90),
                        style,
                        quote.to_bits(),
                        state.position[0],
                        state.position[1],
                        state.width,
                        state.height,
                        state.floors
                    ]
                )
                .value as u8
                    != 0;
                call!(
                    0x4725f0,
                    [word(&player.bytes, 0xb68), word(&player.bytes, 0xb90)]
                );
                return Ok(success);
            }
        }
        call!(
            0x475f70,
            [player.reference, word(&player.bytes, 0xb90), FLOORS, AREA]
        );
        call!(0x46ee90, [word(&player.bytes, 0xb90), AREA]);
        if !fit {
            state.outputs[0] = 0;
        }
    }
    Ok(false)
}

/// Baseline convenience entrypoint. Live scenarios supply their script-modified
/// catalog to `choose_from_catalog`.
pub fn choose(
    player: &mut Player,
    profiles: &mut [[u8; 0x650]],
    categories: &[[u8; 0x17c]],
    image: &PeImage<'_>,
    input: Input,
    state: &mut Planning,
    callback: impl FnMut(
        Call,
        &mut Player,
        &mut [[u8; 0x650]],
        &[[u8; 0x17c]],
        &mut Planning,
    ) -> Result<Reply, String>,
) -> Result<bool, String> {
    choose_from_catalog(
        player,
        profiles,
        categories,
        &crate::business_pool::BusinessCatalog::from_pe(image)?,
        input,
        state,
        callback,
    )
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
        fn planning(&mut self) -> Planning {
            Planning {
                area: self.word(),
                floors: self.word(),
                height: self.word(),
                width: self.word(),
                count: self.word(),
                position: [self.word(), self.word()],
                footprints: std::array::from_fn(|_| self.word()),
                outputs: [self.word(), self.word()],
            }
        }
    }
    #[test]
    #[ignore = "requires original business-planning observations"]
    fn matches_native_planning() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_ACTIONS").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIACT1");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let input = Input {
                execute: r.word() as u8,
                style_override: r.word() as u8,
                style: r.word(),
            };
            let expected = r.word() != 0;
            let outputs = [r.word(), r.word()];
            let count = r.word();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let mut profiles = [r.blob().try_into().unwrap()];
            let categories: Vec<[u8; 0x17c]> = r
                .blob()
                .chunks_exact(0x17c)
                .map(|s| s.try_into().unwrap())
                .collect();
            let after = r.blob();
            let pa = r.blob();
            let ca = r.blob();
            let mut trace = vec![];
            for _ in 0..count {
                let at = r.word();
                let _receiver = r.word();
                let value = r.word();
                let n = r.word();
                let args: Vec<_> = (0..n).map(|_| r.word()).collect();
                let sb = r.planning();
                let sa = r.planning();
                trace.push((
                    at,
                    value,
                    args,
                    sb,
                    sa,
                    r.blob(),
                    r.blob(),
                    r.blob(),
                    r.blob(),
                    r.blob(),
                    r.blob(),
                ));
            }
            let mut state = Planning {
                outputs: [0x1234, 0x5678],
                ..Default::default()
            };
            let mut cursor = 0;
            let result = choose(
                &mut player,
                &mut profiles,
                &categories,
                &image,
                input,
                &mut state,
                |call, player, profiles, categories, state| {
                    let (at, value, args, sb, sa, pb, prof_b, cat_b, pa, prof_a, cat_a) =
                        &trace[cursor];
                    cursor += 1;
                    assert_eq!(call.address, *at, "{name} callback{cursor}");
                    assert_eq!(
                        &call.arguments[..call.count],
                        args,
                        "{name} arguments{at:x}"
                    );
                    assert_eq!(*state, *sb, "{name} planning before{at:x}");
                    assert_eq!(player.bytes, *pb, "{name} player before{at:x}");
                    assert_eq!(
                        profiles[0].as_slice(),
                        *prof_b,
                        "{name} profile before{at:x}"
                    );
                    assert_eq!(
                        categories.as_flattened(),
                        *cat_b,
                        "{name} category before{at:x}"
                    );
                    *state = *sa;
                    player.bytes.copy_from_slice(pa);
                    profiles[0].copy_from_slice(prof_a);
                    assert_eq!(
                        categories.as_flattened(),
                        *cat_a,
                        "{name} categories unchanged by read-only seam"
                    );
                    Ok(Reply {
                        value: *value,
                        quote: f32::from_bits(*value),
                    })
                },
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(result, expected, "{name} result");
            assert_eq!(state.outputs, outputs, "{name} outputs");
            assert_eq!(cursor, trace.len(), "{name} complete callbacks");
            assert_eq!(player.bytes, after, "{name} player final");
            assert_eq!(profiles[0].as_slice(), pa, "{name} profile final");
            assert_eq!(categories.as_flattened(), ca, "{name} category final");
        }
        assert!(r.0.is_empty());
    }
}

pub mod continuation;

pub mod proposals;

pub mod placement;

pub mod demand;

pub mod landscape;

pub mod retained;

pub mod construction;

pub mod queries;

pub mod liquidation;

pub mod performance;

pub mod expansion;
pub mod liquidation_pool;
pub mod liquidation_producer;
pub mod liquidation_score;
