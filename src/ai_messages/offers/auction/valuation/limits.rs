//! Original479150 affordability and479c60 final signed budget adjustment.
use crate::{
    ai_profiles,
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};
fn constant(image: &PeImage<'_>, at: u32) -> Result<f32, String> {
    Ok(f32::from_le_bytes(image.at(at, 4)?.try_into().unwrap()))
}
///479150 preserves the f32 conversion of the integer resource requirement,
///then updates the retained valuation classification only on its state2 path.
pub fn affordability(
    player: &mut Player,
    base: f32,
    resource: i32,
    image: &PeImage<'_>,
) -> Result<Sum, String> {
    let mut factor = Sum::from_f32(ai_profiles::factor(
        image,
        0x47ce90,
        word(&player.bytes, 0x150),
    )?)?;
    let required = Sum::from_i32(resource).to_f32();
    let cash = f32::from_bits(word(&player.bytes, 4));
    if cash > required {
        factor.mul_f32(base)?;
        return Ok(factor);
    }
    if word(&player.bytes, 0x1e30) != 2 {
        return Sum::from_f32(base).map_err(Into::into);
    }
    let mut deficit = Sum::from_f32(required)?;
    deficit.add(Sum::from_f32(cash)?.negated());
    let reduction = deficit.ftol_low_word().min(1000).wrapping_mul(100) / 1000;
    let mut reduction = Sum::from_i32(reduction);
    reduction.mul_f32(base)?;
    reduction.mul_f32(constant(image, 0x5250e0)?)?;
    let mut result = Sum::from_f32(base)?;
    result.add(Sum::from_i32(reduction.ftol_low_word()).negated());
    put(
        &mut player.bytes,
        0x1e30,
        if result
            .compare(Sum::from_f32(constant(image, 0x525040)?)?)
            .is_gt()
        {
            3
        } else {
            u32::MAX
        },
    );
    Ok(result)
}
///479c60. `loan_active` is original4727b0's retained player status byte query.
pub fn final_budget(
    player: &Player,
    group_limit: i32,
    delta: i32,
    loan_active: bool,
    image: &PeImage<'_>,
) -> Result<i32, String> {
    let state = word(&player.bytes, 0x1e30);
    let adjustment = ai_profiles::adjustment(word(&player.bytes, 0x150));
    if group_limit < delta {
        return Ok(group_limit);
    }
    let extra = group_limit
        .wrapping_sub(delta)
        .min(5000i32.wrapping_sub(adjustment));
    if loan_active || state > 2 {
        return Ok(delta);
    }
    let mut weight = Sum::from_i32(1000i32.wrapping_sub(adjustment));
    weight.mul_f32(constant(image, 0x525c94)?)?;
    if state == 1 {
        weight.mul_f32(constant(image, 0x52502c)?)?;
    } else if state == 2 {
        weight.mul_f32(constant(image, 0x525cdc)?)?;
    }
    let stored = Sum::from_i32(delta).to_f32();
    let mut result = Sum::from_i32(extra);
    result.mul(weight);
    result.add_f32(stored)?;
    let result = result.ftol_low_word();
    let mut cap = Sum::from_f32(stored)?;
    cap.mul_f32(constant(image, 0x525cd8)?)?;
    Ok(if Sum::from_i32(result).compare(cap).is_gt() {
        cap.ftol_low_word()
    } else {
        result
    })
}
#[test]
#[ignore = "requires original auction valuation limits capture and executable"]
fn matches_native_limits() {
    fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (a, b) = data.split_at(n);
        *data = b;
        a
    }
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_VALUATION_LIMITS")).unwrap();
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let mut data = &bytes[..];
    assert_eq!(take(&mut data, 8), b"MTAVLIM1");
    assert_eq!(word(take(&mut data, 4), 0), 0x37f);
    let count = word(take(&mut data, 4), 0);
    for case in 0..count {
        let base = f32::from_bits(word(take(&mut data, 4), 0));
        let resource = word(take(&mut data, 4), 0) as i32;
        let group = word(take(&mut data, 4), 0) as i32;
        let delta = word(take(&mut data, 4), 0) as i32;
        let loan = word(take(&mut data, 4), 0) != 0;
        let before = take(&mut data, 0x1e50);
        let mut player = Player {
            reference: 0x10010000,
            bytes: before.to_vec(),
            auxiliaries: vec![],
        };
        let result = affordability(&mut player, base, resource, &image).unwrap();
        assert_eq!(
            &player.bytes[..],
            take(&mut data, 0x1e50),
            "case {case} affordability player"
        );
        assert_eq!(
            result.to_f32().to_bits(),
            word(take(&mut data, 4), 0),
            "case {case} affordability result"
        );
        player.bytes.copy_from_slice(before);
        let result = final_budget(&player, group, delta, loan, &image).unwrap();
        assert_eq!(
            &player.bytes[..],
            take(&mut data, 0x1e50),
            "case {case} final player"
        );
        assert_eq!(
            result as u32,
            word(take(&mut data, 4), 0),
            "case {case} final result"
        );
    }
    assert!(data.is_empty());
}
