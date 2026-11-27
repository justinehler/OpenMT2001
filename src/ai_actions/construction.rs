//! Original48ac20: reprice, pack command0f, then synchronously enqueue.
use crate::{
    pe::PeImage,
    player_manager::{Player, put},
};
/// Return all60 descriptor bytes, preserving native untouched stack padding.
pub fn descriptor(
    player: &Player,
    args: [u32; 11],
    components: [f32; 4],
    mut prior: [u8; 60],
) -> [u8; 60] {
    for (at, v) in [
        (0, 0x525dd4),
        (4, 15),
        (12, 0),
        (0x1c, args[6]),
        (0x20, args[10]),
        (0x24, args[7]),
    ] {
        put(&mut prior, at, v);
    }
    prior[0x10] = 1;
    prior[0x14] = args[0] as u8;
    prior[0x15] = args[1] as u8;
    prior[0x17] = args[2] as u8;
    prior[0x16] = args[1].wrapping_add(args[3]).wrapping_sub(1) as u8;
    prior[0x18] = args[2].wrapping_add(args[4]).wrapping_sub(1) as u8;
    prior[0x19] = args[5] as u8;
    prior[0x28] = args[8] as u8;
    prior[0x29] = player.bytes[0x1bc].wrapping_add(1);
    for (at, value) in [0x2c, 0x38, 0x34, 0x30].into_iter().zip(components) {
        put(&mut prior, at, value.to_bits());
    }
    prior
}
pub fn produce(
    player: &Player,
    args: [u32; 11],
    prior: [u8; 60],
    world: &mut super::retained::World<'_>,
    image: &PeImage<'_>,
) -> Result<[u8; 60], String> {
    let quote = quote_arguments(player, args);
    let cost = super::retained::quote(&quote, player, world, image)?;
    *world.construction_components = cost.components;
    Ok(descriptor(player, args, cost.components, prior))
}

pub fn quote_arguments(player: &Player, args: [u32; 11]) -> [u32; 10] {
    let mut quote = [0; 10];
    quote[..6].copy_from_slice(&[
        args[0],
        args[3].wrapping_mul(args[4]).wrapping_mul(args[5]),
        args[7],
        args[10],
        args[8],
        player.reference,
    ]);
    quote
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires native48ac20 constructor observations"]
    fn matches_native_construction() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_CONSTRUCTION")).unwrap();
        assert_eq!(&data[..8], b"MTAICON1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x37f);
        let n = word(12) as usize;
        assert_eq!(data.len(), 16 + n * 208);
        for i in 0..n {
            let at = 16 + i * 208;
            let mut player = Player {
                reference: 0x10010000,
                bytes: vec![0; 0x1e50],
                auxiliaries: vec![],
            };
            player.bytes[0x1bc] = word(at) as u8;
            let args = std::array::from_fn(|j| word(at + 4 + j * 4));
            let components = std::array::from_fn(|j| f32::from_bits(word(at + 48 + j * 4)));
            let expected: Vec<_> = (0..6).map(|j| word(at + 64 + j * 4)).collect();
            assert_eq!(
                &quote_arguments(&player, args)[..6],
                expected,
                "case{i}quote"
            );
            let prior = data[at + 88..at + 148].try_into().unwrap();
            assert_eq!(
                descriptor(&player, args, components, prior),
                data[at + 148..at + 208],
                "case{i}packet"
            );
        }
    }
}
