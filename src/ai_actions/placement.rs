//! Original4747c0 edge-ordered placement and474e00 edge ordering.
use crate::player_manager::{Player, put, word};
///Invalid native edge codes preserve the caller's complete prior array.
pub fn edge_order(side: u32, forward: u8, output: &mut [u32; 5]) {
    if !(1..=4).contains(&side) {
        return;
    }
    for (i, v) in output.iter_mut().enumerate() {
        *v = if forward != 0 {
            (side - 1 + i as u32) % 4 + 1
        } else {
            (side - 1 + 4 - (i as u32 % 4)) % 4 + 1
        };
    }
}
///The passed player is the original manager lookup for the owner argument.
///Width/height rotations persist even when no placement is found.
pub fn place(
    player: &mut Player,
    position: &mut [u32; 2],
    width: &mut u32,
    height: &mut u32,
    mut available: impl FnMut([i32; 4]) -> Result<bool, String>,
) -> Result<bool, String> {
    if *width == 0 || *height == 0 {
        return Ok(false);
    }
    let direction = player.bytes[0x13f8];
    let mut side = word(&player.bytes, 0x13fc);
    if side == 0 || side == u32::MAX {
        let seed = word(&player.bytes, 0x224)
            .wrapping_mul(word(&player.bytes, 0x228))
            .wrapping_add(word(&player.bytes, 0x22c));
        put(&mut player.bytes, 0x224, seed);
        side = ((seed >> 16) & 0x7fff) % 4 + 1;
        put(&mut player.bytes, 0x13fc, side);
    }
    let mut order = [0; 5];
    edge_order(side, direction, &mut order);
    for edge in order {
        if !(1..=4).contains(&edge) {
            continue;
        }
        if (matches!(edge, 1 | 3) && (*width as i32) > (*height as i32))
            || (matches!(edge, 2 | 4) && (*height as i32) > (*width as i32))
        {
            std::mem::swap(width, height);
        }
        let w = *width as i32;
        let h = *height as i32;
        let horizontal = matches!(edge, 1 | 3);
        let end = 7i32.wrapping_sub(if horizontal { w } else { h });
        if end < 0 {
            continue;
        }
        let increasing = if direction != 0 {
            matches!(edge, 1 | 4)
        } else {
            matches!(edge, 2 | 3)
        };
        let mut along = if increasing { 0 } else { end };
        loop {
            let (x, y) = match edge {
                1 => (along, 7i32.wrapping_sub(h)),
                2 => (7i32.wrapping_sub(w), along),
                3 => (along, 0),
                4 => (0, along),
                _ => unreachable!(),
            };
            if available([x, y, w, h])? {
                *position = [x as u32, y as u32];
                return Ok(true);
            }
            if increasing {
                if along == end {
                    break;
                }
                along = along.wrapping_add(1);
            } else {
                if along == 0 {
                    break;
                }
                along = along.wrapping_sub(1);
            }
        }
    }
    Ok(false)
}
pub fn place_in_category(
    player: &mut Player,
    category: &[u8; 0x17c],
    position: &mut [u32; 2],
    width: &mut u32,
    height: &mut u32,
) -> Result<bool, String> {
    let grid = category[0x9c..0xcd].try_into().unwrap();
    place(player, position, width, height, |[x, y, w, h]| {
        Ok(crate::building_pool::can_place(
            grid,
            [
                x,
                y,
                x.wrapping_add(w).wrapping_sub(1),
                y.wrapping_add(h).wrapping_sub(1),
            ],
        ))
    })
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
    #[ignore = "requires original edge placement observations"]
    fn matches_native_placement() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_PLACEMENT")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIPLC1");
        assert_eq!(r.word(), 0x37f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let entry = r.word();
            let side = r.word();
            let forward = r.word() as u8;
            let expected = r.word() != 0;
            let count = r.word();
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let category = r.blob();
            let mut output = r.blob().to_vec();
            let pa = r.blob();
            let ca = r.blob();
            let oa = r.blob();
            let expected_trace: Vec<[u32; 4]> = (0..count)
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut trace = vec![];
            if entry == 0x474e00 {
                let mut order = std::array::from_fn(|i| word(&output, i * 4));
                edge_order(side, forward, &mut order);
                for (i, v) in order.iter().enumerate() {
                    put(&mut output, i * 4, *v);
                }
            } else {
                let mut position = [word(&output, 0), word(&output, 4)];
                let mut width = word(&output, 8);
                let mut height = word(&output, 12);
                let grid = category[0x9c..0xcd].try_into().unwrap();
                let result = place(
                    &mut player,
                    &mut position,
                    &mut width,
                    &mut height,
                    |args| {
                        trace.push(args.map(|v| v as u32));
                        let [x, y, w, h] = args;
                        Ok(crate::building_pool::can_place(
                            grid,
                            [
                                x,
                                y,
                                x.wrapping_add(w).wrapping_sub(1),
                                y.wrapping_add(h).wrapping_sub(1),
                            ],
                        ))
                    },
                )
                .unwrap();
                assert_eq!(result, expected, "{name} result");
                for (i, v) in [position[0], position[1], width, height].iter().enumerate() {
                    put(&mut output, i * 4, *v);
                }
            }
            assert_eq!(trace, expected_trace, "{name} full placement trace");
            assert_eq!(player.bytes, pa, "{name} player");
            assert_eq!(category, ca, "{name} category");
            assert_eq!(output, oa, "{name} output");
        }
        assert!(r.0.is_empty());
    }
}
