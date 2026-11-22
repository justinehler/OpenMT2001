//! Original42a980 corner brackets. Timer phase stays parked in paused inspection.
#[derive(Debug)]
pub struct Brackets {
    pub rectangles: [[f32; 4]; 8],
    pub color: u32,
    pub counter: i32,
    pub visible: bool,
}
pub fn brackets(point: [f32; 2], counter: i32) -> Brackets {
    let offsets = [
        [-20., -20., -10., -15.],
        [-20., -20., -15., -10.],
        [10., -20., 20., -15.],
        [15., -20., 20., -10.],
        [10., 15., 20., 20.],
        [15., 10., 20., 20.],
        [-20., 15., -10., 20.],
        [-20., 10., -15., 20.],
    ];
    Brackets {
        rectangles: offsets
            .map(|r| std::array::from_fn(|i| (f64::from(point[i % 2]) + r[i]) as f32)),
        color: if counter <= 20 {
            0xffffffff
        } else {
            0x00ffffff
        },
        counter: if counter > 40 { 0 } else { counter },
        visible: counter <= 20,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original strategic selection fixture"]
    fn matches_native_strategic_selection() {
        let b =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_STRATEGIC_SELECTION").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTSELBR1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let count = word(&mut at);
        for _ in 0..count {
            let point = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let input = word(&mut at) as i32;
            let actual = brackets(point, input);
            assert_eq!(actual.counter, word(&mut at) as i32);
            assert_eq!(actual.visible, b[at] != 0);
            at += 1;
            assert_eq!(word(&mut at), 8);
            for r in actual.rectangles {
                for v in r {
                    assert_eq!(v.to_bits(), word(&mut at));
                }
                assert_eq!(actual.color, word(&mut at));
                assert_eq!(word(&mut at), 1.5f32.to_bits());
            }
        }
        assert_eq!(at, b.len());
    }
}
