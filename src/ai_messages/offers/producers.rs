//! Original48a640/48a690 property offer message arguments. Admission, routing,
//! and sender-context lookup occur synchronously through player_messages::produce.
pub fn property(
    address: u32,
    slot: u8,
    category: u32,
    amount_bits: u32,
    identity: u32,
    categories: &[[u8; 380]],
) -> Result<[i32; 8], String> {
    let (recipient, kind) = match address {
        0x48a640 => (
            i32::from(
                categories
                    .get(category as usize)
                    .ok_or("property offer category outside pool")?[5] as i8,
            ) - 1,
            0x23,
        ),
        0x48a690 => (-1, 0x22),
        _ => return Err("unknown property offer producer".into()),
    };
    Ok([
        recipient,
        i32::from(slot),
        kind,
        category as i32,
        crate::economy::ftol_low_word(f64::from(f32::from_bits(amount_bits))),
        identity as i32,
        0,
        -999,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original v1.4 native producer capture"]
    fn matches_native_property_producers() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PROPERTY_PRODUCERS").unwrap())
                .unwrap();
        assert_eq!(&bytes[..8], b"MTAOPR01");
        let mut words = bytes[8..]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        assert_eq!(words.next(), Some(0x037f));
        let count = words.next().unwrap();
        for _ in 0..count {
            let input: [u32; 6] = std::array::from_fn(|_| words.next().unwrap());
            let expected: [i32; 8] = std::array::from_fn(|_| words.next().unwrap() as i32);
            let mut categories = vec![[0; 380]; 52];
            categories[input[2] as usize][5] = input[5] as u8;
            assert_eq!(
                property(
                    input[0],
                    input[1] as u8,
                    input[2],
                    input[3],
                    input[4],
                    &categories
                )
                .unwrap(),
                expected,
                "{input:?}"
            );
        }
        assert_eq!(words.next(), None);
    }
}
