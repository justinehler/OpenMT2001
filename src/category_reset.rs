//! Byte-faithful 00447bf0 second-loop category reset.
//!
//! This models only the pass after existing category lists/buildings have
//! already been removed.  A byte row keeps fields whose reset provenance is
//! not established intact (notably +0x90, +0x94, +0xe0 and +0xe4).

pub const CATEGORY_COUNT: usize = 52;
pub const CATEGORY_ROW_BYTES: usize = 0x17c;
pub const SOURCE_STRIDE: usize = 0x58;

const F32_NEG_41: u32 = 0xc224_0000;
const F32_41: u32 = 0x4224_0000;

fn put_u32(row: &mut [u8; CATEGORY_ROW_BYTES], offset: usize, value: u32) {
    row[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Apply the original second category reset loop to rows whose linked lists
/// have already been removed. `source_values[i]` is the dword read from the
/// original table at `0x5385f4 + i*0x58` and stored at row `+0x84`.
pub fn reset_after_lists_removed(
    rows: &mut [[u8; CATEGORY_ROW_BYTES]; CATEGORY_COUNT],
    source_values: &[u32; CATEGORY_COUNT],
) {
    for (index, (row, &value)) in rows.iter_mut().zip(source_values).enumerate() {
        put_u32(row, 0x15c, 0);
        put_u32(row, 0x160, 0);
        put_u32(row, 0x000, index as u32);
        put_u32(row, 0x008, 0);
        row[0x005] = 0;
        put_u32(row, 0x070, 0);
        row[0x074] = 0;
        put_u32(row, 0x078, 0);
        put_u32(row, 0x07c, 0);
        put_u32(row, 0x080, 0);
        put_u32(row, 0x088, 0);
        row[0x06c] = 0;
        row[0x06d] = 0;
        put_u32(row, 0x084, value);

        // Exact 00447a90 result for its (row+0xe8, 10.0f) call: eight
        // vertices, three dwords each, followed by the scalar bounds. The
        // helper selects +/-41 on X/Z and 0/10 on Y from the vertex index.
        for vertex in 0..8 {
            let x = if vertex & 1 == 0 { F32_NEG_41 } else { F32_41 };
            let y = if vertex & 2 == 0 { 0 } else { 0x4120_0000 };
            let z = if vertex & 4 == 0 { F32_NEG_41 } else { F32_41 };
            let offset = 0x0e8 + vertex * 0x0c;
            put_u32(row, offset, x);
            put_u32(row, offset + 4, y);
            put_u32(row, offset + 8, z);
        }
        put_u32(row, 0x148, 0);
        put_u32(row, 0x14c, 0x40a0_0000); // 5.0f
        put_u32(row, 0x150, 0);
        put_u32(row, 0x154, 0x4553_b000); // 3387.0f
        put_u32(row, 0x158, 0x4268_cab0); // sqrt(3387.0f), original x87 result
    }
}

///00447bd0, called late in fresh startup at004ba15c, after the level and
/// roster initializers. Copies current descriptor prestige without resetting
/// the buildings, residents, ownership, or any other live category fields.
pub fn refresh_prestige(
    rows: &mut [[u8; CATEGORY_ROW_BYTES]; CATEGORY_COUNT],
    values: &[u32; CATEGORY_COUNT],
) {
    for (row, value) in rows.iter_mut().zip(values) {
        put_u32(row, 0x84, *value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original category reset observations"]
    fn matches_native_reset() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CATEGORY_RESET").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTNCATRS");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(
            [word(8), word(12), word(16), word(20)],
            [4, 52, 0x17c, 0x58]
        );
        let mut at = 28;
        for case in 0..word(24) {
            let sources = std::array::from_fn(|i| word(at + i * 4));
            at += 52 * 4;
            let mut rows = std::array::from_fn(|i| {
                bytes[at + i * CATEGORY_ROW_BYTES..at + (i + 1) * CATEGORY_ROW_BYTES]
                    .try_into()
                    .unwrap()
            });
            reset_after_lists_removed(&mut rows, &sources);
            at += CATEGORY_COUNT * CATEGORY_ROW_BYTES;
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(
                    row.as_slice(),
                    &bytes[at + i * CATEGORY_ROW_BYTES..at + (i + 1) * CATEGORY_ROW_BYTES],
                    "case {case} complete category row {i}"
                );
            }
            at += CATEGORY_COUNT * CATEGORY_ROW_BYTES;
            let sources = std::array::from_fn(|i| word(at + i * 4));
            at += 52 * 4;
            refresh_prestige(&mut rows, &sources);
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(
                    row.as_slice(),
                    &bytes[at + i * CATEGORY_ROW_BYTES..at + (i + 1) * CATEGORY_ROW_BYTES],
                    "case {case} refreshed category row {i}"
                );
            }
            at += CATEGORY_COUNT * CATEGORY_ROW_BYTES;
        }
        assert_eq!(at, bytes.len());
    }
}
