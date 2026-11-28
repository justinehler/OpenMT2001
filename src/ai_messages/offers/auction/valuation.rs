//! Original46d7a0 valuation orchestration. World-query and scoring helpers stay
//! explicit until their original implementations are bound.
pub mod base;
pub mod limits;
pub mod scoring;
use crate::{
    player_manager::{Player, put, word},
    x87::Sum,
};
#[derive(Clone, Copy, Debug)]
pub struct Call {
    pub address: u32,
    pub args: [u32; 4],
    pub count: usize,
}
#[derive(Clone, Copy, Default)]
pub struct Reply {
    pub integer: u32,
    pub extended: Sum,
}
pub fn evaluate(
    player: &mut Player,
    mode: u32,
    auction_category: Option<u32>,
    percent: f32,
    output: &mut i32,
    mut callback: impl FnMut(Call, &mut Player) -> Result<Reply, String>,
) -> Result<Sum, String> {
    *output = 0;
    put(&mut player.bytes, 0x1e30, u32::MAX);
    let category = if mode == 1 {
        auction_category.ok_or("auction valuation allocation missing")?
    } else if mode == 0 || mode == 2 {
        word(&player.bytes, 0xb68)
    } else {
        return Err("auction valuation mode unsupported".into());
    };
    let mut call = |address, args: &[u32], player: &mut Player| {
        let mut padded = [0; 4];
        padded[..args.len()].copy_from_slice(args);
        callback(
            Call {
                address,
                args: padded,
                count: args.len(),
            },
            player,
        )
    };
    if mode != 1 {
        call(0x4477e0, &[category], player)?;
    }
    let flag = u32::from(mode == 1);
    let mut base = call(
        0x4790c0,
        &[player.bytes[0x1bc] as u32, flag, category],
        player,
    )?
    .extended
    .to_f32();
    let resource = call(0x479040, &[category, player.bytes[0x1bc] as u32], player)?.integer;
    base = call(
        0x479150,
        &[player.bytes[0x1bc] as u32, base.to_bits(), resource],
        player,
    )?
    .extended
    .to_f32();
    let repeated = call(0x478ff0, &[category, player.bytes[0x1bc] as u32], player)?.integer;
    base = call(
        0x479280,
        &[
            category,
            player.bytes[0x1bc] as u32,
            base.to_bits(),
            repeated,
        ],
        player,
    )?
    .extended
    .to_f32();
    base = call(
        0x4793c0,
        &[player.bytes[0x1bc] as u32, category, base.to_bits()],
        player,
    )?
    .extended
    .to_f32();
    let mut cumulative = call(
        0x479460,
        &[flag, player.bytes[0x1bc] as u32, category, base.to_bits()],
        player,
    )?
    .extended
    .to_f32();
    let group = call(0x479640, &[category, player.bytes[0x1bc] as u32], player)?.integer;
    let delta = call(0x4796f0, &[category, player.bytes[0x1bc] as u32], player)?
        .extended
        .ftol_low_word();
    base = call(
        0x4797a0,
        &[category, player.bytes[0x1bc] as u32, cumulative.to_bits()],
        player,
    )?
    .extended
    .to_f32();
    cumulative = call(
        0x479810,
        &[
            category,
            player.bytes[0x1bc] as u32,
            cumulative.to_bits(),
            base.to_bits(),
        ],
        player,
    )?
    .extended
    .to_f32();
    cumulative = call(
        0x4799c0,
        &[
            category,
            player.bytes[0x1bc] as u32,
            cumulative.to_bits(),
            resource,
        ],
        player,
    )?
    .extended
    .to_f32();
    cumulative = call(
        0x479bc0,
        &[category, player.bytes[0x1bc] as u32, cumulative.to_bits()],
        player,
    )?
    .extended
    .to_f32();
    let delta = call(
        0x479c60,
        &[player.bytes[0x1bc] as u32, group, delta as u32],
        player,
    )?
    .integer as i32;
    let mut product = Sum::from_i32(delta);
    product.mul_f32(cumulative)?;
    product.mul_f32(percent)?;
    let mut extra = Sum::from_f32(cumulative)?;
    extra.mul_f32(percent)?;
    product.add(extra);
    *output = product.ftol_low_word();
    Ok(product)
}
#[test]
#[ignore = "requires original auction valuation pipeline capture"]
fn matches_native_pipeline() {
    let d = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_VALUATION_PIPELINE")).unwrap();
    let mut a = 8;
    assert_eq!(&d[..8], b"MTAUVP01");
    fn w(d: &[u8], a: &mut usize) -> u32 {
        let v = u32::from_le_bytes(d[*a..*a + 4].try_into().unwrap());
        *a += 4;
        v
    }
    assert_eq!(w(&d, &mut a), 0x37f);
    let n = w(&d, &mut a);
    let percent = f32::from_bits(w(&d, &mut a));
    for case in 0..n {
        let mode = w(&d, &mut a);
        let category = w(&d, &mut a);
        let mut p = Player {
            reference: 0x10010000,
            auxiliaries: vec![],
            bytes: d[a..a + 0x1e50].to_vec(),
        };
        a += 0x1e50;
        let after = &d[a..a + 0x1e50];
        a += 0x1e50;
        let integer = w(&d, &mut a);
        let bits = w(&d, &mut a);
        let count = w(&d, &mut a);
        let mut traces = Vec::new();
        for _ in 0..count {
            let length = w(&d, &mut a);
            traces.push((0..length).map(|_| w(&d, &mut a)).collect::<Vec<_>>());
        }
        let mut index = 0;
        let mut output = 333;
        let r = evaluate(
            &mut p,
            mode,
            Some(category),
            percent,
            &mut output,
            |call, p| {
                let t = &traces[index];
                assert_eq!(call.address, t[0], "case {case} helper {index}");
                assert_eq!(call.count, t[1] as usize);
                assert_eq!(
                    &call.args[..call.count],
                    &t[2..2 + call.count],
                    "case {case} helper {index} args"
                );
                if case % 7 == 0 {
                    p.bytes[0x1bc] = if p.bytes[0x1bc] < 5 {
                        p.bytes[0x1bc] + 1
                    } else {
                        0
                    };
                    put(&mut p.bytes, 0x1e30, index as u32);
                }
                index += 1;
                Ok(Reply {
                    integer: t[t.len() - 2],
                    extended: Sum::from_f32(f32::from_bits(t[t.len() - 1])).unwrap(),
                })
            },
        )
        .unwrap();
        assert_eq!(index, traces.len());
        assert_eq!(output as u32, integer, "case {case} integer");
        assert_eq!(r.to_f32().to_bits(), bits, "case {case} f32");
        assert_eq!(p.bytes, after, "case {case} player");
    }
    assert_eq!(a, d.len());
}
