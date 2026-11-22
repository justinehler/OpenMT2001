//! Original470af0 daily AI bookkeeping, including46afa0 and467190.
use crate::player_manager::{Player, put, word};
pub fn advance(
    players: &mut [Player],
    ownership: &mut crate::ai_ownership::Storage,
) -> Result<(), String> {
    for player in players {
        if player.bytes.len() < 0x158 || player.bytes[0x10] == 0 || word(&player.bytes, 0x154) != 1
        {
            continue;
        }
        if player.bytes.len() < 0x1e50 {
            return Err("daily AI allocation truncated".into());
        }
        let allocated = usize::from(u16::from_le_bytes(
            player.bytes[0xb46..0xb48].try_into().unwrap(),
        ));
        if allocated != 0 {
            let rows = ownership
                .lists
                .get_mut(&word(&player.bytes, 0xb3c))
                .ok_or("daily AI ownership allocation missing")?;
            for row in rows
                .get_mut(..allocated)
                .ok_or("daily AI ownership allocation truncated")?
            {
                let age = word(row, 8).wrapping_add(1);
                put(row, 8, age);
            }
        }
        let days = word(&player.bytes, 0xc98).wrapping_add(1);
        put(&mut player.bytes, 0xc98, days);
        //470b37 repeatedly reads the current outer player, not the inner-loop
        //peer. Its slot always equals ESI's slot, so the cd4 store is skipped.
        for id in 0..52 {
            let at = 0x1158 + id * 12;
            let age = word(&player.bytes, at);
            if age as i32 > 0 {
                put(&mut player.bytes, at, age.wrapping_sub(1));
            }
        }
    }
    Ok(())
}
#[test]
#[ignore = "requires complete original470af0 daily observations"]
fn matches_native_daily() {
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_DAILY").unwrap()).unwrap();
    assert_eq!(&data[..8], b"MTAIDAY1");
    assert_eq!(word(&data, 8), 0x37f);
    let mut at = 16;
    for case in 0..word(&data, 12) {
        let mut blobs = vec![];
        for _ in 0..4 {
            let n = word(&data, at) as usize;
            at += 4;
            blobs.push(&data[at..at + n]);
            at += n;
        }
        let mut players: Vec<_> = blobs[0]
            .chunks_exact(0x1e50)
            .enumerate()
            .map(|(i, b)| Player {
                reference: 0x10010000 + i as u32 * 0x2000,
                bytes: b.to_vec(),
                auxiliaries: vec![],
            })
            .collect();
        let mut storage = crate::ai_ownership::Storage::default();
        for (i, b) in blobs[1].chunks_exact(6 * 28).enumerate() {
            storage.lists.insert(
                0x10030000 + i as u32 * 0x1000,
                b.chunks_exact(28).map(|r| r.try_into().unwrap()).collect(),
            );
        }
        advance(&mut players, &mut storage).unwrap();
        for (i, (a, b)) in players
            .iter()
            .flat_map(|p| p.bytes.iter())
            .zip(blobs[2])
            .enumerate()
        {
            assert_eq!(a, b, "case{case} playerbyte{i:x}");
        }
        for (i, (a, b)) in storage
            .lists
            .values()
            .flatten()
            .flatten()
            .zip(blobs[3])
            .enumerate()
        {
            assert_eq!(a, b, "case{case} listbyte{i:x}");
        }
    }
    assert_eq!(at, data.len());
}
