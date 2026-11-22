//! Original AI profile difficulty-row binding, without host Lua or file I/O.

/// Original4d7346..4d7406 numeric character globals, in profile-index order.
pub const CHARACTER_NAMES: [&str; 10] = [
    "CAR", "BARROW", "DOG", "GUN", "HAT", "HORSE", "IRON", "SHIP", "SHOE", "THIMBLE",
];

/// Original7f92d8..7f92f5. Properties are byte-spaced but4d6570 registers
/// DWORD targets: writes overlap adjacent properties, and reads are signed32.
#[derive(Default)]
pub struct Scores {
    pub bytes: [u8; 29],
}

pub const SCORE_PROPERTIES: [(&str, usize); 25] = [
    ("blk2_Monopoly", 0),
    ("blk2_1left_city_owned", 1),
    ("blk2_opponent_monopoly", 2),
    ("blk2_Max_LandRent_Income", 3),
    ("blk3_Monopoly", 4),
    ("blk3_1left_city_owned", 5),
    ("blk3_2left_city_owned", 6),
    ("blk3_1left_player_owned", 7),
    ("blk3_opponent_monopoly", 8),
    ("blk3_opponent_2blocks", 9),
    ("blk3_Max_Monopoly", 10),
    ("blk3_Max_LandRent_Income", 11),
    ("RRUtil_Monopoly", 12),
    ("RRUtil_1left_city_owned", 13),
    ("RRUtil_2left_city_owned", 14),
    ("RRUtil_3left_city_owned", 15),
    ("RRUtil_3left_Bonus", 16),
    ("RRUtil_opponent_monopoly", 17),
    ("RRUtil_Max_LandRent_Income", 18),
    ("MaxScoreCap", 19),
    ("MaxDevelopment", 21),
    ("MaxObjective", 22),
    ("MaxHindrance", 23),
    ("BasicBidVariance", 24),
    ("MaxTTscoreThreshold", 25),
];
impl Scores {
    pub fn set(&mut self, property: usize, value: f64) -> Result<(), String> {
        let offset = SCORE_PROPERTIES
            .get(property)
            .ok_or("unknown AI score property")?
            .1;
        self.bytes[offset..offset + 4]
            .copy_from_slice(&crate::economy::ftol_low_word(value).to_le_bytes());
        Ok(())
    }
    pub fn get(&self, property: usize) -> Result<i32, String> {
        let offset = SCORE_PROPERTIES
            .get(property)
            .ok_or("unknown AI score property")?
            .1;
        Ok(i32::from_le_bytes(
            self.bytes[offset..offset + 4].try_into().unwrap(),
        ))
    }
}

/// Original466b10/466b80. This is a partial reset, not a zero-filled profile.
/// `disabled` is player+1400; call-site selection remains the host's decision.
pub fn reset_profile(profile: &mut [u8; 0x650], disabled: &mut u8) {
    *disabled = 1;
    profile[4] = 0;
    for offset in [0, 0xc, 0x10, 0x28, 0x1c] {
        profile[offset..offset + 4].fill(0);
    }
    profile[0x36..0x40].fill(0);
    for offset in [8, 0xf0, 0x410, 0x280] {
        profile[offset..offset + 4].fill(0xff);
    }
}

/// Original4d77c0: truncate the Lua number and retain its low byte.
pub fn set_base_value_multiplier(profile: &mut [u8; 0x650], value: f64) {
    profile[0x5a0] = crate::economy::ftol_low_word(value) as u8;
}

/// Original4d7730 stores the full FTOL low dword, rather than a byte.
pub fn set_prestige_bias(profile: &mut [u8; 0x650], value: f64) {
    profile[0x28..0x2c].copy_from_slice(&crate::economy::ftol_low_word(value).to_le_bytes());
}

/// Original4d6da0 stores four independent preference bytes without normalizing.
pub fn set_strategy_bias(profile: &mut [u8; 0x650], values: [f64; 4]) {
    for (destination, value) in profile[0x5a1..0x5a5].iter_mut().zip(values) {
        *destination = crate::economy::ftol_low_word(value) as u8;
    }
}

/// Original4d7930 uses current-hour global822950 and wrapping signed arithmetic.
/// The three intermediate minute arguments are read then discarded natively.
pub fn set_build_times(
    profile: &mut [u8; 0x650],
    values: [f64; 10],
    current_hour: i32,
) -> Result<(), String> {
    let integer = |i| crate::economy::ftol_low_word(values[i]);
    let offset = usize::try_from(0x614_i64 + i64::from(integer(0)) * 20)
        .map_err(|_| "AI build-time row leaves profile")?;
    let row = profile
        .get_mut(offset..offset.saturating_add(20))
        .ok_or("AI build-time row leaves profile")?;
    let hour = integer(1);
    let mut delta = hour.wrapping_sub(current_hour);
    if hour < current_hour {
        delta = delta.wrapping_add(24);
    }
    let elapsed = delta
        .wrapping_mul(625)
        .wrapping_add(integer(2).wrapping_mul(10));
    for (destination, value) in
        row.chunks_exact_mut(4)
            .zip([elapsed, integer(3), integer(5), integer(7), integer(9)])
    {
        destination.copy_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

/// Original4d6e10: ten floor preference bytes, with FTOL then low-byte stores.
pub fn set_floors(profile: &mut [u8; 0x650], arguments: [f64; 10]) {
    for (destination, value) in profile[0x36..0x40].iter_mut().zip(arguments) {
        *destination = crate::economy::ftol_low_word(value) as u8;
    }
}

/// Original46b900 catalog. Both helper arguments are interpreted as bytes.
/// Unsupported pairs return -1; the original also reports them to its logger.
pub fn footprint_area(footprint: i32, floor: i32) -> i32 {
    const AREAS: [[i32; 10]; 5] = [
        [1, 2, 3, 4, 4, -1, -1, -1, -1, -1],
        [2, 3, 4, 5, 4, 6, 8, -1, -1, -1],
        [4, 6, 9, -1, -1, -1, -1, -1, -1, -1],
        [6, 8, 10, 9, 12, 15, 16, -1, -1, -1],
        [8, 10, 12, 9, 12, 15, 18, 16, 20, 24],
    ];
    AREAS
        .get(footprint as u8 as usize)
        .and_then(|row| row.get(floor as u8 as usize))
        .copied()
        .unwrap_or(-1)
}

/// Original4d6e60. Preserve the padding and third byte of each area subentry.
pub fn set_footprint(profile: &mut [u8; 0x650], arguments: [f64; 4]) -> Result<(), String> {
    let index = crate::economy::ftol_low_word(arguments[0]);
    let offset = 0x40_i64 + i64::from(index) * 35;
    let offset = usize::try_from(offset).map_err(|_| "AI footprint leaves profile")?;
    let row = profile
        .get_mut(offset..offset.saturating_add(35))
        .ok_or("AI footprint leaves profile")?;
    for (destination, value) in row[..3].iter_mut().zip(&arguments[1..]) {
        *destination = crate::economy::ftol_low_word(*value) as u8;
    }
    row[3] = index as u8;
    for floor in 0..10 {
        let area = footprint_area(index, floor as i32) as u8;
        row[5 + floor * 3] = u8::from(area != 0);
        row[6 + floor * 3] = area;
    }
    Ok(())
}

/// Original4d77f0 after extracting Lua arguments2..17. Integer stores use the
/// original FTOL low word and byte/word truncation; padding remains untouched.
pub fn set_difficulty(profile: &mut [u8; 0x650], arguments: [f64; 16]) -> Result<(), String> {
    let difficulty = crate::economy::ftol_low_word(arguments[0]);
    let offset = (0x5a8_i64 + i64::from(difficulty) * 36) as usize;
    let row = profile
        .get_mut(offset..offset.saturating_add(36))
        .ok_or("AI difficulty row leaves profile")?;
    let values = &arguments[1..];
    for (index, at) in [
        (0, 0),
        (1, 1),
        (6, 18),
        (7, 19),
        (8, 20),
        (9, 21),
        (10, 22),
        (11, 23),
        (14, 32),
    ] {
        row[at] = crate::economy::ftol_low_word(values[index]) as u8;
    }
    row[16..18].copy_from_slice(&(crate::economy::ftol_low_word(values[5]) as u16).to_le_bytes());
    for (index, at) in [(2, 4), (3, 8), (4, 12), (12, 24), (13, 28)] {
        row[at..at + 4].copy_from_slice(&(values[index] as f32).to_le_bytes());
    }
    Ok(())
}

/// Original47d9f0 after selecting player+150's profile and player+13f4's row.
pub fn rent_increase(profile: &[u8; 0x650], difficulty: i32) -> Result<i32, String> {
    let offset = (0x5b0_i64 + i64::from(difficulty) * 36) as usize;
    let bytes = profile
        .get(offset..offset.saturating_add(4))
        .ok_or("AI rent difficulty leaves profile")?;
    Ok(crate::economy::ftol_low_word(f64::from(
        f32::from_le_bytes(bytes.try_into().unwrap()),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original4d77c0 multiplier observations"]
    fn matches_native_base_multiplier() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_SCORES").unwrap()).unwrap();
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAISCR1");
        assert_eq!(word(8), 1);
        assert_eq!(word(16), 0x650);
        let mut at = 20;
        for _ in 0..word(12) {
            let value = f64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
            assert_eq!(word(at + 12), 0x100f0000);
            assert_eq!(word(at + 16), 0);
            assert_eq!(word(at + 20), 0x650);
            at += 24;
            let mut profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
            at += 0x650;
            set_base_value_multiplier(&mut profile, value);
            assert_eq!(profile.as_slice(), &bytes[at..at + 0x650]);
            at += 0x650;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original profile reset observations"]
    fn matches_native_profile_reset() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PROFILE_INIT").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTAIPRF2");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(16), word(20)), (1, 10, 0x650));
        let mut at = 24;
        for case in 0..word(12) {
            at += 12;
            let mut profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
            let expected = &bytes[at + 0x650..at + 0x650 * 2];
            at += 0x650 * 2;
            let mut player = bytes[at..at + 0x20].to_vec();
            reset_profile(&mut profile, &mut player[0]);
            assert_eq!(profile.as_slice(), expected, "reset profile {case}");
            assert_eq!(player, &bytes[at + 0x20..at + 0x40], "reset player {case}");
            at += 0x40;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original SetBuildTimes observations"]
    fn matches_native_build_times() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_BUILD_TIMES").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTAIBT01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let current = word(at + 4) as i32;
            at += 8;
            let values = std::array::from_fn(|i| {
                f64::from_le_bytes(bytes[at + i * 8..at + i * 8 + 8].try_into().unwrap())
            });
            at += 80;
            let mut profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
            at += 0x650;
            set_build_times(&mut profile, values, current).unwrap();
            assert_eq!(
                profile.as_slice(),
                &bytes[at..at + 0x650],
                "build times {case}"
            );
            at += 0x650;
            assert_eq!(word(at), 0);
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original prestige and strategy setter observations"]
    fn matches_native_biases() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_BIASES").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTAIBIA1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 1);
        assert_eq!(word(16), 0x650);
        let mut at = 20;
        for case in 0..word(12) {
            let kind = word(at);
            let count = word(at + 8);
            at += 12;
            let values = std::array::from_fn(|i| {
                f64::from_le_bytes(bytes[at + i * 8..at + i * 8 + 8].try_into().unwrap())
            });
            at += 32;
            let mut profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
            at += 0x650;
            match kind {
                0 => {
                    assert_eq!(count, 1);
                    set_prestige_bias(&mut profile, values[0]);
                }
                1 => {
                    assert_eq!(count, 4);
                    set_strategy_bias(&mut profile, values);
                }
                _ => panic!("unknown bias setter"),
            }
            assert_eq!(profile.as_slice(), &bytes[at..at + 0x650], "bias {case}");
            at += 0x650;
            assert_eq!(word(at), 0);
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original preference setter and area helper observations"]
    fn matches_native_preferences() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PREFERENCES").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTAIPRF1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 1);
        let mut at = 20;
        for case in 0..word(12) {
            let kind = word(at);
            let count = word(at + 8) as usize;
            at += 12;
            let arguments: [f64; 16] = std::array::from_fn(|i| {
                f64::from_le_bytes(bytes[at + i * 8..at + i * 8 + 8].try_into().unwrap())
            });
            at += 128;
            let mut profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
            at += 0x650;
            match kind {
                0 => {
                    assert_eq!(count, 10);
                    set_floors(&mut profile, arguments[..10].try_into().unwrap());
                }
                1 => {
                    assert_eq!(count, 4);
                    set_footprint(&mut profile, arguments[..4].try_into().unwrap()).unwrap();
                }
                _ => panic!("unknown preference setter"),
            }
            assert_eq!(
                profile.as_slice(),
                &bytes[at..at + 0x650],
                "preference {case}"
            );
            at += 0x650;
            assert_eq!(word(at), 0);
            at += 4;
        }
        for _ in 0..word(16) {
            assert_eq!(
                footprint_area(word(at) as i32, word(at + 4) as i32),
                word(at + 8) as i32
            );
            at += 12;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original difficulty setter and rent getter observations"]
    fn matches_native_parameters() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PARAMETERS").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTAIPAR1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            // Native fixture varies selected profile slots; production receives
            // the corresponding profile directly, without an original pointer.
            at += 4;
            let arguments = std::array::from_fn(|i| {
                f64::from_le_bytes(bytes[at + i * 8..at + i * 8 + 8].try_into().unwrap())
            });
            at += 128;
            let mut profile: [u8; 0x650] = bytes[at..at + 0x650].try_into().unwrap();
            at += 0x650;
            set_difficulty(&mut profile, arguments).unwrap();
            let differences: Vec<_> = profile
                .iter()
                .zip(&bytes[at..at + 0x650])
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .take(16)
                .collect();
            assert!(differences.is_empty(), "parameter{case}: {differences:?}");
            at += 0x650;
            assert_eq!(
                rent_increase(&profile, crate::economy::ftol_low_word(arguments[0])).unwrap(),
                word(at) as i32,
                "parameter{case}"
            );
            at += 4;
        }
        assert_eq!(at, bytes.len());
    }
}
