//! Original468de0/46ec80 business-kind selection before construction planning.
use crate::{
    ai_selection::{Time, windows},
    player_manager::{Player, put, word},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Plan,
    HousingEligible,
    Rank(i32),
    Diagnostic,
}

///46ec80. A housing probe precedes retail ranking; failed probes retain every
/// native planning side effect except the explicitly reset business kind.
pub fn preferred(
    player: &mut Player,
    mut call: impl FnMut(Call, &mut Player, &mut [u32; 2]) -> Result<u32, String>,
) -> Result<i32, String> {
    let mut outputs = [u32::MAX; 2];
    if call(Call::HousingEligible, player, &mut outputs)? as u8 != 0 {
        put(&mut player.bytes, 0xb90, 42);
        if call(Call::Plan, player, &mut outputs)? as u8 != 0 {
            return Ok(42);
        }
        put(&mut player.bytes, 0xb90, u32::MAX);
    }
    let result = call(Call::Rank(-1), player, &mut outputs)? as i32;
    if result == 44 {
        call(Call::Diagnostic, player, &mut outputs)?;
    }
    Ok(result)
}

///468de0. The category's active/mode checks precede resetting the chosen kind.
pub fn select(
    player: &mut Player,
    profiles: &[[u8; 0x650]],
    categories: &[[u8; 380]; 52],
    time: Time,
    mut call: impl FnMut(Call, &mut Player, &mut [u32; 2]) -> Result<u32, String>,
) -> Result<bool, String> {
    let category = word(&player.bytes, 0xb68) as i32;
    if category <= -1 {
        return Ok(false);
    }
    let row = categories
        .get(category as usize)
        .ok_or("AI business selection category leaves pool")?;
    let owned = player.bytes[0x178 + category as usize] != 0;
    let mode = word(row, 0x78);
    if (mode == 1 && !owned) || mode == 0 || row[0x6c] != 0 {
        player.bytes[0xb76] = 1;
        return Ok(false);
    }
    put(&mut player.bytes, 0xb90, u32::MAX);
    match word(&player.bytes, 0x1040) {
        3 if owned => {
            put(&mut player.bytes, 0xb90, 44);
            // Native arguments point to stack+0x10 (status=-1), then +0xc(cost=0).
            let mut outputs = [u32::MAX, 0];
            if call(Call::Plan, player, &mut outputs)? as u8 == 0 {
                put(&mut player.bytes, 0xb90, u32::MAX);
            }
        }
        mode @ (0 | 1) => {
            let gates = windows(player, profiles, time.hour)?;
            if if mode == 0 { gates.0 } else { gates.1 } {
                let kind = preferred(player, &mut call)?;
                put(&mut player.bytes, 0xb90, kind as u32);
            }
        }
        _ => {}
    }
    if word(&player.bytes, 0xb90) == u32::MAX {
        player.bytes[0xb76] = 1;
        Ok(false)
    } else {
        Ok(true)
    }
}

#[test]
#[ignore = "requires original business-choice observations"]
fn matches_native_business_choice() {
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
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_BUSINESS_CHOICE")).unwrap();
    let mut r = Reader(&bytes);
    assert_eq!(r.take(8), b"MTBCHO01");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let hour = r.word() as i32;
        let expected = r.word();
        let count = r.word();
        let mut player = Player {
            reference: 0x10010000,
            bytes: r.blob().to_vec(),
            auxiliaries: vec![],
        };
        let after = r.blob();
        let categories: Vec<[u8; 380]> = r
            .blob()
            .chunks_exact(380)
            .map(|b| b.try_into().unwrap())
            .collect();
        let mut trace = vec![];
        for _ in 0..count {
            let h: [u32; 4] = std::array::from_fn(|_| r.word());
            trace.push((h, r.blob(), r.blob()));
        }
        let mut profile = [0u8; 0x650];
        for (i, v) in [6, 12, 20].into_iter().enumerate() {
            put(&mut profile, 0x618 + i * 4, v);
        }
        let mut cursor = 0;
        let result = select(
            &mut player,
            &[profile],
            categories.as_slice().try_into().unwrap(),
            Time { hour, year: 0 },
            |call, p, outputs| {
                let (h, b, a) = &trace[cursor];
                cursor += 1;
                assert_eq!(
                    match call {
                        Call::Plan => 0x47c630,
                        Call::HousingEligible => 0x4771f0,
                        Call::Rank(-1) => 0x46e6a0,
                        Call::Diagnostic => 0x4e6960,
                        _ => panic!("unexpected rank"),
                    },
                    h[0],
                    "case{case} call"
                );
                assert_eq!(p.bytes, *b, "case{case} beforecall");
                if call == Call::Plan {
                    assert_eq!(*outputs, [h[2], h[3]], "case{case} outputs");
                    *outputs = [77, 78];
                }
                p.bytes.copy_from_slice(a);
                Ok(h[1])
            },
        )
        .unwrap();
        assert_eq!(u32::from(result), expected, "case{case} result");
        assert_eq!(player.bytes, after, "case{case} player");
        assert_eq!(cursor, trace.len(), "case{case} trace");
    }
    assert!(r.0.is_empty());
}
